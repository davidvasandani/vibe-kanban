use std::{sync::Arc, time::Duration};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
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

/// Handles bidirectional control protocol communication
#[derive(Clone)]
pub struct ProtocolPeer {
    stdin: Arc<Mutex<ChildStdin>>,
}

impl ProtocolPeer {
    pub fn spawn(
        stdin: ChildStdin,
        stdout: ChildStdout,
        client: Arc<ClaudeAgentClient>,
        cancel: CancellationToken,
    ) -> Self {
        let peer = Self {
            stdin: Arc::new(Mutex::new(stdin)),
        };

        let reader_peer = peer.clone();
        tokio::spawn(async move {
            if let Err(e) = reader_peer.read_loop(stdout, client, cancel).await {
                tracing::error!("Protocol reader loop error: {}", e);
            }
        });

        peer
    }

    async fn read_loop(
        &self,
        stdout: ChildStdout,
        client: Arc<ClaudeAgentClient>,
        cancel: CancellationToken,
    ) -> Result<(), ExecutorError> {
        let mut reader = BufReader::new(stdout);
        let mut buffer = String::new();
        let mut interrupt_sent = false;
        // Set once the final `result` arrives; when it expires we break, which
        // drops stdin and lets the CLI exit. Until then keep answering
        // trailing control requests (e.g. the Stop hook fired after the
        // result), which would otherwise fail with "Stream closed" in the CLI.
        let mut grace_deadline: Option<Instant> = None;

        loop {
            buffer.clear();
            tokio::select! {
                biased;
                _ = cancel.cancelled(), if !interrupt_sent => {
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
                line_result = reader.read_line(&mut buffer) => {
                    match line_result {
                        Ok(0) => break, // EOF
                        Ok(_) => {
                            let line = buffer.trim();
                            if line.is_empty() {
                                continue;
                            }
                            client.log_message(line).await?;

                            // Parse and handle control messages
                            match serde_json::from_str::<CLIMessage>(line) {
                                Ok(CLIMessage::ControlRequest {
                                    request_id,
                                    request,
                                }) => {
                                    self.handle_control_request(&client, request_id, request)
                                        .await;
                                }
                                Ok(CLIMessage::Result(_)) => {
                                    grace_deadline
                                        .get_or_insert_with(|| Instant::now() + POST_RESULT_GRACE);
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
    ) {
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
                        if let Err(e) = self
                            .send_hook_response(request_id, serde_json::to_value(result).unwrap())
                            .await
                        {
                            tracing::error!("Failed to send permission result: {e}");
                        }
                    }
                    Err(ExecutorError::ExecutorApprovalError(ExecutorApprovalError::Cancelled)) => {
                    }
                    Err(e) => {
                        tracing::error!("Error in on_can_use_tool: {e}");
                        if let Err(e2) = self.send_error(request_id, e.to_string()).await {
                            tracing::error!("Failed to send error response: {e2}");
                        }
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
                        if let Err(e) = self.send_hook_response(request_id, hook_output).await {
                            tracing::error!("Failed to send hook callback result: {e}");
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error in on_hook_callback: {e}");
                        if let Err(e2) = self.send_error(request_id, e.to_string()).await {
                            tracing::error!("Failed to send error response: {e2}");
                        }
                    }
                }
            }
        }
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
