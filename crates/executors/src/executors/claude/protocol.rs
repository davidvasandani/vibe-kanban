use std::{sync::Arc, time::Duration};

use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    process::{ChildStdin, ChildStdout},
    sync::Mutex,
    time::{Instant, sleep_until},
};
use tokio_util::sync::CancellationToken;

use super::types::{CLIMessage, ControlRequestType, ControlResponseMessage, ControlResponseType};
use crate::{
    approvals::ExecutorApprovalError,
    executors::{
        ExecutorError,
        claude::{
            client::ClaudeAgentClient,
            types::{Message, PermissionMode, SDKControlRequest, SDKControlRequestType},
        },
    },
};

/// How long to keep answering control requests after the final `result`
/// message. The CLI can fire the Stop hook after emitting the result (e.g.
/// when interrupted); closing stdin immediately would make that callback fail
/// with "Stream closed" inside the CLI.
const POST_RESULT_GRACE: Duration = Duration::from_millis(500);

/// How long to wait for real turn activity after ignoring a spurious
/// zero-turn result before giving up and ending the session anyway.
const SPURIOUS_RESULT_FALLBACK: Duration = Duration::from_secs(30);

/// Handles bidirectional control protocol communication
#[derive(Clone)]
pub struct ProtocolPeer {
    stdin: Arc<Mutex<Box<dyn AsyncWrite + Send + Unpin>>>,
}

impl ProtocolPeer {
    pub fn spawn(
        stdin: ChildStdin,
        stdout: ChildStdout,
        client: Arc<ClaudeAgentClient>,
        cancel: CancellationToken,
    ) -> Self {
        let peer = Self::with_writer(stdin);

        let reader_peer = peer.clone();
        tokio::spawn(async move {
            if let Err(e) = reader_peer.read_loop(stdout, client, cancel).await {
                tracing::error!("Protocol reader loop error: {}", e);
            }
        });

        peer
    }

    fn with_writer(stdin: impl AsyncWrite + Send + Unpin + 'static) -> Self {
        Self {
            stdin: Arc::new(Mutex::new(Box::new(stdin))),
        }
    }

    async fn read_loop<R>(
        &self,
        stdout: R,
        client: Arc<ClaudeAgentClient>,
        cancel: CancellationToken,
    ) -> Result<(), ExecutorError>
    where
        R: AsyncRead + Unpin,
    {
        let mut reader = BufReader::new(stdout);
        let mut buffer = String::new();
        let mut interrupt_sent = false;
        // Set once the final `result` arrives; when it expires we break, which
        // drops stdin and lets the CLI exit. Until then keep answering
        // trailing control requests (e.g. the Stop hook fired after the
        // result), which would otherwise fail with "Stream closed" in the CLI.
        let mut grace_deadline: Option<Instant> = None;
        // Set when a spurious zero-turn result is ignored. If the real turn
        // hasn't started by the time it fires, treat the run as finished
        // instead of keeping stdin open forever. Cleared on any sign of turn
        // activity.
        let mut spurious_fallback: Option<Instant> = None;

        loop {
            buffer.clear();
            tokio::select! {
                biased;
                // Once a terminal result armed the grace window the turn is
                // already over; sending interrupt then would only race the
                // trailing Stop hook the grace window exists to serve.
                _ = cancel.cancelled(), if !interrupt_sent && grace_deadline.is_none() => {
                    interrupt_sent = true;
                    tracing::info!("Cancellation received in read_loop, sending interrupt to Claude");
                    if let Err(e) = self.interrupt().await {
                        tracing::warn!("Failed to send interrupt to Claude: {e}");
                    }
                    // Continue the loop to read Claude's response (it should send a result)
                }
                _ = sleep_until(grace_deadline.unwrap_or_else(Instant::now)), if grace_deadline.is_some() => {
                    break;
                }
                _ = sleep_until(spurious_fallback.unwrap_or_else(Instant::now)),
                    if spurious_fallback.is_some() && grace_deadline.is_none() => {
                    tracing::warn!(
                        "No turn activity after ignored zero-turn result; ending session"
                    );
                    break;
                }
                line_result = reader.read_line(&mut buffer) => {
                    match line_result {
                        Ok(0) => break, // EOF
                        Ok(_) => {
                            let line = buffer.trim();
                            if line.is_empty() {
                                continue;
                            }

                            // A result starts a *quiescence* window, not an
                            // absolute teardown timer. Claude may still emit
                            // control traffic while background SDK work drains;
                            // every line proves the stream is active and buys a
                            // complete quiet interval for the next callback.
                            if grace_deadline.is_some() {
                                grace_deadline = Some(Instant::now() + POST_RESULT_GRACE);
                            }

                            // Parse before logging so the spurious result
                            // below can be kept out of the user-facing log
                            // (it would otherwise render as an empty
                            // assistant message).
                            let parsed = serde_json::from_str::<CLIMessage>(line);

                            // claude-code >= 2.1.200 can emit a spurious
                            // zero-turn success result immediately after
                            // resuming a session with queued task
                            // notifications, before it has processed our
                            // prompt. Treating it as terminal closes stdin
                            // and silently swallows the request, so keep
                            // reading; the real turn produces its own
                            // result. After an interrupt a zero-turn result
                            // is legitimate (nothing ran).
                            if let Ok(CLIMessage::Result(result)) = &parsed
                                && !interrupt_sent
                                && result.get("num_turns").and_then(|v| v.as_u64()) == Some(0)
                                && !result
                                    .get("is_error")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false)
                            {
                                tracing::warn!(
                                    "Ignoring zero-turn success result (resume artifact); continuing to read"
                                );
                                spurious_fallback
                                    .get_or_insert_with(|| Instant::now() + SPURIOUS_RESULT_FALLBACK);
                                continue;
                            }

                            client.log_message(line).await?;

                            match parsed {
                                Ok(CLIMessage::ControlRequest {
                                    request_id,
                                    request,
                                }) => {
                                    // Tool activity means the real turn is running.
                                    spurious_fallback = None;
                                    self.handle_control_request(&client, request_id, request)
                                        .await?;
                                }
                                Ok(CLIMessage::Result(_)) => {
                                    spurious_fallback = None;
                                    grace_deadline.get_or_insert_with(|| {
                                        Instant::now() + POST_RESULT_GRACE
                                    });
                                }
                                Ok(CLIMessage::Other(value)) => {
                                    // Replayed/synthetic messages are resume
                                    // history, not evidence that the real turn
                                    // started; they must not clear the fallback.
                                    let replayed = ["isReplay", "isSynthetic"].iter().any(|k| {
                                        value.get(k).and_then(|v| v.as_bool()).unwrap_or(false)
                                    });
                                    if !replayed
                                        && matches!(
                                            value.get("type").and_then(|t| t.as_str()),
                                            Some("assistant" | "stream_event" | "user")
                                        )
                                    {
                                        spurious_fallback = None;
                                    }
                                }
                                _ => {}
                            }
                        }
                        Err(e) => {
                            tracing::error!("Error reading stdout: {}", e);
                            break;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_control_request(
        &self,
        client: &Arc<ClaudeAgentClient>,
        request_id: String,
        request: ControlRequestType,
    ) -> Result<(), ExecutorError> {
        match request {
            ControlRequestType::CanUseTool {
                tool_name,
                input,
                permission_suggestions,
                blocked_paths: _,
                tool_use_id,
            } => {
                match client
                    .on_can_use_tool(tool_name, input, permission_suggestions, tool_use_id)
                    .await
                {
                    Ok(result) => {
                        self.send_hook_response(request_id, serde_json::to_value(result).unwrap())
                            .await?;
                    }
                    Err(ExecutorError::ExecutorApprovalError(ExecutorApprovalError::Cancelled)) => {
                    }
                    Err(e) => {
                        tracing::error!("Error in on_can_use_tool: {e}");
                        self.send_error(request_id, e.to_string()).await?;
                    }
                }
            }
            ControlRequestType::HookCallback {
                callback_id,
                input,
                tool_use_id,
            } => {
                match client
                    .on_hook_callback(callback_id, input, tool_use_id)
                    .await
                {
                    Ok(hook_output) => {
                        self.send_hook_response(request_id, hook_output).await?;
                    }
                    Err(e) => {
                        tracing::error!("Error in on_hook_callback: {e}");
                        self.send_error(request_id, e.to_string()).await?;
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn send_hook_response(
        &self,
        request_id: String,
        hook_output: serde_json::Value,
    ) -> Result<(), ExecutorError> {
        self.send_json(&ControlResponseMessage::new(ControlResponseType::Success {
            request_id,
            response: Some(hook_output),
        }))
        .await
    }

    /// Send error response to CLI
    async fn send_error(&self, request_id: String, error: String) -> Result<(), ExecutorError> {
        self.send_json(&ControlResponseMessage::new(ControlResponseType::Error {
            request_id,
            error: Some(error),
        }))
        .await
    }

    async fn send_json<T: serde::Serialize>(&self, message: &T) -> Result<(), ExecutorError> {
        let json = serde_json::to_string(message)?;
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(json.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        Ok(())
    }

    pub async fn send_user_message(&self, content: String) -> Result<(), ExecutorError> {
        let message = Message::new_user(content);
        self.send_json(&message).await
    }

    pub async fn initialize(&self, hooks: Option<serde_json::Value>) -> Result<(), ExecutorError> {
        self.send_json(&SDKControlRequest::new(SDKControlRequestType::Initialize {
            hooks,
        }))
        .await
    }
    pub async fn interrupt(&self) -> Result<(), ExecutorError> {
        self.send_json(&SDKControlRequest::new(SDKControlRequestType::Interrupt {}))
            .await
    }

    pub async fn set_permission_mode(&self, mode: PermissionMode) -> Result<(), ExecutorError> {
        self.send_json(&SDKControlRequest::new(
            SDKControlRequestType::SetPermissionMode { mode },
        ))
        .await
    }
}

#[cfg(test)]
mod tests {
    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader, duplex},
        time::{Duration, sleep, timeout},
    };

    use super::*;
    use crate::{env::RepoContext, executors::codex::client::LogWriter};

    fn test_client() -> Arc<ClaudeAgentClient> {
        ClaudeAgentClient::new(
            LogWriter::new(tokio::io::sink()),
            None,
            RepoContext::default(),
            String::new(),
            CancellationToken::new(),
            Arc::new(crate::executors::claude::ClaudeMcpInventory::default()),
        )
    }

    /// Same callback id, but a foreground command that is an unbounded wait.
    /// The `command` carries the shape from the incident.
    const UNBOUNDED_WAIT_HOOK_REQUEST: &[u8] =
        br#"{"type":"control_request","request_id":"wait-1","request":{"subtype":"hook_callback","callback_id":"DENY_BACKGROUND_BASH_CALLBACK_ID","input":{"tool_input":{"command":"until grep -q ready /tmp/out.log; do sleep 5; done"}}}}"#;

    /// A value test on the deny JSON does not prove the refusal reaches the
    /// CLI — the contract is what crosses the boundary, so drive it over the
    /// real duplex protocol and read the matching `control_response`.
    #[tokio::test]
    async fn unbounded_wait_hook_is_denied_over_the_protocol() {
        let (mut cli_stdout, vk_stdout) = duplex(16 * 1024);
        let (vk_stdin, cli_stdin) = duplex(16 * 1024);
        let peer = ProtocolPeer::with_writer(vk_stdin);
        let cancel = CancellationToken::new();
        let reader =
            tokio::spawn(async move { peer.read_loop(vk_stdout, test_client(), cancel).await });

        cli_stdout
            .write_all(UNBOUNDED_WAIT_HOOK_REQUEST)
            .await
            .unwrap();
        cli_stdout.write_all(b"\n").await.unwrap();

        let mut response = String::new();
        timeout(
            Duration::from_millis(250),
            BufReader::new(cli_stdin).read_line(&mut response),
        )
        .await
        .expect("hook response should be prompt")
        .expect("hook response should be readable");
        let response: serde_json::Value = serde_json::from_str(response.trim()).unwrap();
        assert_eq!(response["type"], "control_response");
        assert_eq!(response["response"]["request_id"], "wait-1");
        assert_eq!(
            response["response"]["response"]["hookSpecificOutput"]["permissionDecision"],
            "deny"
        );
        assert!(
            response["response"]["response"]["hookSpecificOutput"]["permissionDecisionReason"]
                .as_str()
                .unwrap()
                .contains("spawn_poller"),
            "the delivered denial must name the replacement"
        );

        drop(cli_stdout);
        reader.await.unwrap().unwrap();
    }
    const BACKGROUND_BASH_HOOK_REQUEST: &[u8] =
        br#"{"type":"control_request","request_id":"deny-1","request":{"subtype":"hook_callback","callback_id":"DENY_BACKGROUND_BASH_CALLBACK_ID","input":{"tool_input":{"run_in_background":true}}}}"#;

    #[tokio::test]
    async fn post_result_activity_keeps_background_bash_hook_stream_open() {
        let (mut cli_stdout, vk_stdout) = duplex(16 * 1024);
        let (vk_stdin, cli_stdin) = duplex(16 * 1024);
        let peer = ProtocolPeer::with_writer(vk_stdin);
        let cancel = CancellationToken::new();
        let reader =
            tokio::spawn(async move { peer.read_loop(vk_stdout, test_client(), cancel).await });

        cli_stdout
            .write_all(b"{\"type\":\"result\",\"num_turns\":1,\"is_error\":false}\n")
            .await
            .unwrap();
        sleep(Duration::from_millis(350)).await;
        cli_stdout
            .write_all(b"{\"type\":\"assistant\",\"message\":{}}\n")
            .await
            .unwrap();

        // The hook arrives after the original absolute 500 ms post-result
        // deadline, but less than one quiet interval after the latest output.
        sleep(Duration::from_millis(250)).await;
        cli_stdout
            .write_all(BACKGROUND_BASH_HOOK_REQUEST)
            .await
            .expect("protocol input remains open after post-result activity");
        cli_stdout.write_all(b"\n").await.unwrap();

        let mut response = String::new();
        timeout(
            Duration::from_millis(250),
            BufReader::new(cli_stdin).read_line(&mut response),
        )
        .await
        .expect("hook response should be prompt")
        .expect("hook response should be readable");
        let response: serde_json::Value = serde_json::from_str(response.trim()).unwrap();
        assert_eq!(response["type"], "control_response");
        assert_eq!(response["response"]["request_id"], "deny-1");
        assert_eq!(
            response["response"]["response"]["hookSpecificOutput"]["permissionDecision"],
            "deny"
        );
        assert!(
            response["response"]["response"]["hookSpecificOutput"]["permissionDecisionReason"]
                .as_str()
                .unwrap()
                .contains("spawn_poller")
        );

        drop(cli_stdout);
        reader.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn post_result_quiescence_still_ends_the_protocol_loop() {
        let (mut cli_stdout, vk_stdout) = duplex(1024);
        let (vk_stdin, _cli_stdin) = duplex(1024);
        let peer = ProtocolPeer::with_writer(vk_stdin);
        let reader = tokio::spawn(async move {
            peer.read_loop(vk_stdout, test_client(), CancellationToken::new())
                .await
        });

        cli_stdout
            .write_all(b"{\"type\":\"result\",\"num_turns\":1,\"is_error\":false}\n")
            .await
            .unwrap();

        timeout(Duration::from_millis(1_500), reader)
            .await
            .expect("a quiet completed turn must not keep stdin open indefinitely")
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn hook_response_write_failure_fails_the_protocol_loop() {
        let (mut cli_stdout, vk_stdout) = duplex(4096);
        let (vk_stdin, cli_stdin) = duplex(4096);
        drop(cli_stdin);
        let peer = ProtocolPeer::with_writer(vk_stdin);
        let reader = tokio::spawn(async move {
            peer.read_loop(vk_stdout, test_client(), CancellationToken::new())
                .await
        });

        cli_stdout
            .write_all(BACKGROUND_BASH_HOOK_REQUEST)
            .await
            .unwrap();
        cli_stdout.write_all(b"\n").await.unwrap();

        let error = timeout(Duration::from_millis(250), reader)
            .await
            .expect("failed response write should end the protocol loop")
            .unwrap()
            .expect_err("closed response stream must be a protocol error");
        assert!(
            error.to_string().contains("broken pipe") || error.to_string().contains("closed"),
            "unexpected protocol error: {error:?}"
        );
    }
}
