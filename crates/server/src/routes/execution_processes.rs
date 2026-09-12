use std::collections::HashSet;

use anyhow;
use axum::{
    Extension, Router,
    extract::{
        Path, Query, State,
        ws::{CloseFrame, Message},
    },
    middleware::from_fn_with_state,
    response::{IntoResponse, Json as ResponseJson},
    routing::{get, post},
};
use db::models::{
    execution_process::{ExecutionProcess, ExecutionProcessStatus},
    execution_process_repo_state::ExecutionProcessRepoState,
    execution_worker_job::ExecutionWorkerJob,
};
use deployment::Deployment;
use executors::logs::{NormalizedEntry, NormalizedEntryType};
use futures_util::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use services::services::container::ContainerService;
use ts_rs::TS;
use utils::{log_msg::LogMsg, response::ApiResponse};
use uuid::Uuid;

use crate::{
    DeploymentImpl,
    error::ApiError,
    middleware::{
        load_execution_process_middleware,
        signed_ws::{MaybeSignedWebSocket, SignedWsUpgrade},
    },
};

/// Default/maximum number of messages a `.../messages` request returns, and
/// the per-message character cap. Kept small on purpose: this response is
/// meant for an orchestrator to read in one shot, not to replace the
/// normalized-logs websocket for full-transcript viewing.
const DEFAULT_MESSAGE_LIMIT: usize = 20;
const MAX_MESSAGE_LIMIT: usize = 100;
const MAX_MESSAGE_CHARS: usize = 4000;

#[derive(Debug, Deserialize)]
pub struct RecentMessagesQuery {
    pub limit: Option<usize>,
    /// Return the complete available normalized projection instead of a
    /// bounded recent tail. `limit` is ignored when this is true.
    #[serde(default)]
    pub all: bool,
    /// Comma-separated subset of `user`, `assistant`, `system`, `tool`.
    pub roles: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessagesSelection {
    Recent(usize),
    All,
}

impl RecentMessagesQuery {
    pub fn selection(&self) -> MessagesSelection {
        if self.all {
            MessagesSelection::All
        } else {
            MessagesSelection::Recent(clamp_message_limit(self.limit))
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct SessionMessage {
    pub id: String,
    pub role: String,
    pub text: String,
    pub created_at: Option<String>,
    pub execution_id: String,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct RecentMessagesResponse {
    pub session_id: String,
    pub execution_id: String,
    pub status: ExecutionProcessStatus,
    pub exit_code: Option<i64>,
    /// Last non-empty assistant text for this execution, or `None` if the
    /// turn never produced one (e.g. it errored before responding).
    pub final_message: Option<String>,
    pub messages: Vec<SessionMessage>,
    /// True if there were more matching messages than `limit` allowed
    /// through; `messages` holds the newest `limit` of them.
    pub has_more: bool,
}

/// Maps a normalized entry's type to the coarse role an orchestrator asked
/// to filter by. `None` means the entry isn't a message worth surfacing here
/// (e.g. a loading spinner or token-usage telemetry frame).
fn entry_role(entry_type: &NormalizedEntryType) -> Option<&'static str> {
    match entry_type {
        NormalizedEntryType::UserMessage
        | NormalizedEntryType::UserFeedback { .. }
        | NormalizedEntryType::UserAnsweredQuestions { .. } => Some("user"),
        NormalizedEntryType::AssistantMessage | NormalizedEntryType::Thinking => Some("assistant"),
        NormalizedEntryType::ToolUse { .. } => Some("tool"),
        NormalizedEntryType::SystemMessage
        | NormalizedEntryType::ErrorMessage { .. }
        | NormalizedEntryType::NextAction { .. } => Some("system"),
        NormalizedEntryType::Loading | NormalizedEntryType::TokenUsageInfo(_) => None,
    }
}

fn truncate(text: &str) -> String {
    if text.chars().count() <= MAX_MESSAGE_CHARS {
        return text.to_string();
    }
    let head: String = text.chars().take(MAX_MESSAGE_CHARS).collect();
    format!("{head}\n… [truncated]")
}

/// The last non-empty assistant text among `entries`, truncated like any
/// other message. This is what makes `get_execution.final_message` and
/// `RecentMessagesResponse.final_message` non-null after a real turn.
pub fn last_assistant_message(entries: &[NormalizedEntry]) -> Option<String> {
    entries.iter().rev().find_map(|entry| {
        if !matches!(entry.entry_type, NormalizedEntryType::AssistantMessage) {
            return None;
        }
        let content = entry.content.trim();
        (!content.is_empty()).then(|| truncate(content))
    })
}

pub fn parse_roles(roles: Option<&str>) -> Option<HashSet<String>> {
    let roles = roles?;
    let set: HashSet<String> = roles
        .split(',')
        .map(|role| role.trim().to_ascii_lowercase())
        .filter(|role| !role.is_empty())
        .collect();
    (!set.is_empty()).then_some(set)
}

/// Builds the `.../messages` response for `execution_process`, reusing
/// `ContainerService::normalized_entries` (itself a `.collect()` over the
/// same pipeline `stream_normalized_logs` serves to the websocket) rather
/// than a second store.
pub async fn build_recent_messages_response(
    deployment: &DeploymentImpl,
    execution_process: &ExecutionProcess,
    selection: MessagesSelection,
    roles: Option<&HashSet<String>>,
) -> RecentMessagesResponse {
    let entries = deployment
        .container()
        .normalized_entries(&execution_process.id)
        .await
        .unwrap_or_default();

    let final_message = last_assistant_message(&entries);

    let (messages, has_more) = project_messages(&entries, execution_process.id, selection, roles);

    RecentMessagesResponse {
        session_id: execution_process.session_id.to_string(),
        execution_id: execution_process.id.to_string(),
        status: execution_process.status.clone(),
        exit_code: execution_process.exit_code,
        final_message,
        messages,
        has_more,
    }
}

fn project_messages(
    entries: &[NormalizedEntry],
    execution_id: Uuid,
    selection: MessagesSelection,
    roles: Option<&HashSet<String>>,
) -> (Vec<SessionMessage>, bool) {
    let mut messages: Vec<SessionMessage> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let role = entry_role(&entry.entry_type)?;
            if roles.is_some_and(|roles| !roles.contains(role)) {
                return None;
            }
            let text = entry.content.trim();
            if text.is_empty() {
                return None;
            }
            Some(SessionMessage {
                id: format!("{execution_id}:{index}"),
                role: role.to_string(),
                text: truncate(text),
                created_at: entry.timestamp.clone(),
                execution_id: execution_id.to_string(),
            })
        })
        .collect();

    match selection {
        MessagesSelection::Recent(limit) => {
            let has_more = messages.len() > limit;
            if has_more {
                messages = messages.split_off(messages.len() - limit);
            }
            (messages, has_more)
        }
        MessagesSelection::All => (messages, false),
    }
}

pub fn clamp_message_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(DEFAULT_MESSAGE_LIMIT)
        .clamp(1, MAX_MESSAGE_LIMIT)
}

#[derive(Debug, Deserialize)]
struct SessionExecutionProcessQuery {
    pub session_id: Uuid,
    /// If true, include soft-deleted (dropped) processes in results/stream
    #[serde(default)]
    pub show_soft_deleted: Option<bool>,
}

async fn get_execution_process_by_id(
    Extension(execution_process): Extension<ExecutionProcess>,
    State(_deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<ExecutionProcess>>, ApiError> {
    Ok(ResponseJson(ApiResponse::success(execution_process)))
}

async fn get_execution_process_messages(
    Extension(execution_process): Extension<ExecutionProcess>,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<RecentMessagesQuery>,
) -> Result<ResponseJson<ApiResponse<RecentMessagesResponse>>, ApiError> {
    let selection = query.selection();
    let roles = parse_roles(query.roles.as_deref());
    let response =
        build_recent_messages_response(&deployment, &execution_process, selection, roles.as_ref())
            .await;
    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn get_execution_worker_job(
    State(deployment): State<DeploymentImpl>,
    Extension(execution_process): Extension<ExecutionProcess>,
) -> Result<ResponseJson<ApiResponse<Option<ExecutionWorkerJob>>>, ApiError> {
    let job = ExecutionWorkerJob::find_by_execution_id(&deployment.db().pool, execution_process.id)
        .await?;
    Ok(ResponseJson(ApiResponse::success(job)))
}

async fn stream_raw_logs_ws(
    ws: SignedWsUpgrade,
    State(deployment): State<DeploymentImpl>,
    Path(exec_id): Path<Uuid>,
) -> impl IntoResponse {
    // Always accept the WebSocket upgrade — handle "not found" inside the
    // connection by sending `finished` and closing cleanly, instead of
    // rejecting with HTTP 404 which the browser surfaces as an opaque
    // connection failure.
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_raw_logs_ws(socket, deployment, exec_id).await {
            tracing::warn!("raw logs WS closed: {}", e);
        }
    })
}

async fn handle_raw_logs_ws(
    mut socket: MaybeSignedWebSocket,
    deployment: DeploymentImpl,
    exec_id: Uuid,
) -> anyhow::Result<()> {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use executors::logs::utils::patch::ConversationPatch;
    use utils::log_msg::LogMsg;

    // Get the raw stream — if not found, send finished and close cleanly
    let raw_stream = match deployment.container().stream_raw_logs(&exec_id).await {
        Some(stream) => stream,
        None => {
            // No logs available: send finished so the client gets a clean
            // close instead of retrying endlessly.
            let _ = socket
                .send(LogMsg::Finished.to_ws_message_unchecked())
                .await;
            let _ = socket.close().await;
            return Ok(());
        }
    };

    let counter = Arc::new(AtomicUsize::new(0));
    let mut stream = raw_stream.map_ok({
        let counter = counter.clone();
        move |m| match m {
            LogMsg::Stdout(content) => {
                let index = counter.fetch_add(1, Ordering::SeqCst);
                let patch = ConversationPatch::add_stdout(index, content);
                LogMsg::JsonPatch(patch).to_ws_message_unchecked()
            }
            LogMsg::Stderr(content) => {
                let index = counter.fetch_add(1, Ordering::SeqCst);
                let patch = ConversationPatch::add_stderr(index, content);
                LogMsg::JsonPatch(patch).to_ws_message_unchecked()
            }
            LogMsg::Finished => LogMsg::Finished.to_ws_message_unchecked(),
            _ => unreachable!("Raw stream should only have Stdout/Stderr/Finished"),
        }
    });

    loop {
        tokio::select! {
            item = stream.next() => {
                match item {
                    Some(Ok(msg)) => {
                        if socket.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        tracing::error!("stream error: {}", e);
                        break;
                    }
                    None => break,
                }
            }
            inbound = socket.recv() => {
                match inbound {
                    Ok(Some(Message::Close(_))) => break,
                    Ok(Some(_)) => {}
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        }
    }
    // Send a proper close frame so the client sees code 1000 (normal closure)
    // instead of an abnormal TCP drop that triggers reconnection attempts.
    let _ = socket.close().await;
    Ok(())
}

async fn stream_normalized_logs_ws(
    ws: SignedWsUpgrade,
    State(deployment): State<DeploymentImpl>,
    Path(exec_id): Path<Uuid>,
) -> impl IntoResponse {
    ws.on_upgrade(move |mut socket| async move {
        let stream = tokio::select! {
            stream = deployment.container().stream_normalized_logs(&exec_id) => stream,
            inbound = socket.recv() => {
                tracing::debug!(
                    execution_id = %exec_id,
                    ?inbound,
                    "normalized logs WS closed before historical replay started"
                );
                let _ = socket.close().await;
                return;
            }
        };

        match stream {
            Some(stream) => {
                let stream = stream.err_into::<anyhow::Error>().into_stream();
                if let Err(e) = handle_normalized_logs_ws(socket, stream).await {
                    tracing::warn!("normalized logs WS closed: {}", e);
                }
            }
            None => {
                // No logs available: send finished and close cleanly
                let mut socket = socket;
                let _ = socket
                    .send(utils::log_msg::LogMsg::Finished.to_ws_message_unchecked())
                    .await;
                let _ = socket.close().await;
            }
        }
    })
}

async fn handle_normalized_logs_ws(
    mut socket: MaybeSignedWebSocket,
    stream: impl futures_util::Stream<Item = anyhow::Result<LogMsg>> + Unpin + Send + 'static,
) -> anyhow::Result<()> {
    let mut stream = stream.map_ok(|msg| msg.to_ws_message_unchecked());
    loop {
        tokio::select! {
            item = stream.next() => {
                match item {
                    Some(Ok(msg)) => {
                        if socket.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        tracing::error!("stream error: {}", e);
                        break;
                    }
                    None => break,
                }
            }
            inbound = socket.recv() => {
                match inbound {
                    Ok(Some(Message::Close(_))) => break,
                    Ok(Some(_)) => {}
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        }
    }
    let _ = socket.close().await;
    Ok(())
}

async fn stop_execution_process(
    Extension(execution_process): Extension<ExecutionProcess>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    deployment
        .container()
        .stop_execution(&execution_process, ExecutionProcessStatus::Killed)
        .await?;

    Ok(ResponseJson(ApiResponse::success(())))
}

async fn stream_execution_processes_by_session_ws(
    ws: SignedWsUpgrade,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<SessionExecutionProcessQuery>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_execution_processes_by_session_ws(
            socket,
            deployment,
            query.session_id,
            query.show_soft_deleted.unwrap_or(false),
        )
        .await
        {
            tracing::warn!("execution processes by session WS closed: {}", e);
        }
    })
}

async fn handle_execution_processes_by_session_ws(
    mut socket: MaybeSignedWebSocket,
    deployment: DeploymentImpl,
    session_id: uuid::Uuid,
    show_soft_deleted: bool,
) -> anyhow::Result<()> {
    // Get the raw stream and convert LogMsg to WebSocket messages
    let mut stream = deployment
        .events()
        .stream_execution_processes_for_session_raw(session_id, show_soft_deleted)
        .await?
        .map_ok(|msg| msg.to_ws_message_unchecked());

    loop {
        tokio::select! {
            item = stream.next() => {
                match item {
                    Some(Ok(msg)) => {
                        if socket.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        tracing::error!("stream error: {}", e);
                        // A lagged execution stream is no longer authoritative.
                        // Close with an error code so the client reconnects and
                        // receives a fresh full snapshot instead of retaining a
                        // stale running process forever.
                        let _ = socket
                            .send(Message::Close(Some(resnapshot_close_frame())))
                            .await;
                        return Err(e.into());
                    }
                    None => break,
                }
            }
            inbound = socket.recv() => {
                match inbound {
                    Ok(Some(Message::Close(_))) => break,
                    Ok(Some(_)) => {}
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        }
    }
    Ok(())
}

fn resnapshot_close_frame() -> CloseFrame {
    CloseFrame {
        code: 1011,
        reason: "execution process stream requires resnapshot".into(),
    }
}

async fn get_execution_process_repo_states(
    Extension(execution_process): Extension<ExecutionProcess>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<ExecutionProcessRepoState>>>, ApiError> {
    let pool = &deployment.db().pool;
    let repo_states =
        ExecutionProcessRepoState::find_by_execution_process_id(pool, execution_process.id).await?;
    Ok(ResponseJson(ApiResponse::success(repo_states)))
}

pub(super) fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    let workspace_id_router = Router::new()
        .route("/", get(get_execution_process_by_id))
        .route("/stop", post(stop_execution_process))
        .route("/repo-states", get(get_execution_process_repo_states))
        .route("/worker-job", get(get_execution_worker_job))
        .route("/messages", get(get_execution_process_messages))
        .route("/raw-logs/ws", get(stream_raw_logs_ws))
        .route("/normalized-logs/ws", get(stream_normalized_logs_ws))
        .layer(from_fn_with_state(
            deployment.clone(),
            load_execution_process_middleware,
        ));

    let workspaces_router = Router::new()
        .route(
            "/stream/session/ws",
            get(stream_execution_processes_by_session_ws),
        )
        .nest("/{id}", workspace_id_router);

    Router::new().nest("/execution-processes", workspaces_router)
}

#[cfg(test)]
mod stream_close_tests {
    use super::resnapshot_close_frame;

    #[test]
    fn authority_loss_uses_retryable_resnapshot_close() {
        let frame = resnapshot_close_frame();
        assert_eq!(frame.code, 1011);
        assert!(frame.reason.contains("requires resnapshot"));
    }
}

#[cfg(test)]
mod tests {
    use executors::logs::{ActionType, ToolStatus};

    use super::*;

    fn entry(entry_type: NormalizedEntryType, content: &str) -> NormalizedEntry {
        NormalizedEntry {
            timestamp: None,
            entry_type,
            content: content.to_string(),
            metadata: None,
        }
    }

    #[test]
    fn last_assistant_message_skips_trailing_non_assistant_entries() {
        let entries = vec![
            entry(NormalizedEntryType::AssistantMessage, "first reply"),
            entry(
                NormalizedEntryType::ToolUse {
                    tool_name: "bash".to_string(),
                    action_type: ActionType::Search {
                        query: "grep".to_string(),
                    },
                    status: ToolStatus::Success,
                },
                "ran a command",
            ),
        ];

        assert_eq!(
            last_assistant_message(&entries).as_deref(),
            Some("first reply")
        );
    }

    #[test]
    fn last_assistant_message_ignores_empty_assistant_text() {
        let entries = vec![
            entry(NormalizedEntryType::AssistantMessage, "real answer"),
            entry(NormalizedEntryType::AssistantMessage, "   "),
        ];

        assert_eq!(
            last_assistant_message(&entries).as_deref(),
            Some("real answer")
        );
    }

    #[test]
    fn last_assistant_message_is_none_without_any_assistant_entry() {
        let entries = vec![entry(NormalizedEntryType::UserMessage, "do the thing")];
        assert_eq!(last_assistant_message(&entries), None);
    }

    #[test]
    fn last_assistant_message_truncates_huge_text() {
        let huge = "x".repeat(MAX_MESSAGE_CHARS + 500);
        let entries = vec![entry(NormalizedEntryType::AssistantMessage, &huge)];

        let message = last_assistant_message(&entries).unwrap();
        assert!(message.len() < huge.len());
        assert!(message.ends_with("[truncated]"));
    }

    #[test]
    fn entry_role_maps_conversational_types_and_drops_telemetry() {
        assert_eq!(entry_role(&NormalizedEntryType::UserMessage), Some("user"));
        assert_eq!(
            entry_role(&NormalizedEntryType::AssistantMessage),
            Some("assistant")
        );
        assert_eq!(
            entry_role(&NormalizedEntryType::ToolUse {
                tool_name: "bash".to_string(),
                action_type: ActionType::Search {
                    query: "grep".to_string()
                },
                status: ToolStatus::Success,
            }),
            Some("tool")
        );
        assert_eq!(
            entry_role(&NormalizedEntryType::SystemMessage),
            Some("system")
        );
        assert_eq!(entry_role(&NormalizedEntryType::Loading), None);
    }

    #[test]
    fn parse_roles_lowercases_trims_and_drops_empty() {
        let roles = parse_roles(Some(" Assistant, tool ,,")).unwrap();
        assert_eq!(
            roles,
            HashSet::from(["assistant".to_string(), "tool".to_string()])
        );
    }

    #[test]
    fn parse_roles_is_none_for_absent_or_blank_input() {
        assert_eq!(parse_roles(None), None);
        assert_eq!(parse_roles(Some("  ,  ")), None);
    }

    #[test]
    fn clamp_message_limit_defaults_and_caps() {
        assert_eq!(clamp_message_limit(None), DEFAULT_MESSAGE_LIMIT);
        assert_eq!(clamp_message_limit(Some(0)), 1);
        assert_eq!(clamp_message_limit(Some(9999)), MAX_MESSAGE_LIMIT);
        assert_eq!(clamp_message_limit(Some(5)), 5);
    }

    #[test]
    fn query_selects_all_or_a_clamped_recent_tail() {
        assert_eq!(
            RecentMessagesQuery {
                limit: Some(9999),
                all: false,
                roles: None,
            }
            .selection(),
            MessagesSelection::Recent(MAX_MESSAGE_LIMIT)
        );
        assert_eq!(
            RecentMessagesQuery {
                limit: Some(1),
                all: true,
                roles: None,
            }
            .selection(),
            MessagesSelection::All
        );
    }

    #[test]
    fn all_selection_returns_more_than_recent_cap_in_order() {
        let execution_id = Uuid::new_v4();
        let entries: Vec<_> = (0..125)
            .map(|index| {
                entry(
                    NormalizedEntryType::AssistantMessage,
                    &format!("message {index}"),
                )
            })
            .collect();

        let (all, all_has_more) =
            project_messages(&entries, execution_id, MessagesSelection::All, None);
        let (recent, recent_has_more) = project_messages(
            &entries,
            execution_id,
            MessagesSelection::Recent(MAX_MESSAGE_LIMIT),
            None,
        );

        assert_eq!(all.len(), 125);
        assert_eq!(
            all.first().map(|message| message.text.as_str()),
            Some("message 0")
        );
        assert_eq!(
            all.last().map(|message| message.text.as_str()),
            Some("message 124")
        );
        assert!(!all_has_more);
        assert_eq!(recent.len(), MAX_MESSAGE_LIMIT);
        assert_eq!(
            recent.first().map(|message| message.text.as_str()),
            Some("message 25")
        );
        assert_eq!(
            recent.last().map(|message| message.text.as_str()),
            Some("message 124")
        );
        assert!(recent_has_more);
    }

    #[test]
    fn all_selection_filters_roles_without_marking_more() {
        let execution_id = Uuid::new_v4();
        let entries = vec![
            entry(NormalizedEntryType::UserMessage, "question"),
            entry(NormalizedEntryType::AssistantMessage, "answer"),
            entry(NormalizedEntryType::SystemMessage, "notice"),
        ];
        let roles = HashSet::from(["assistant".to_string()]);

        let (messages, has_more) =
            project_messages(&entries, execution_id, MessagesSelection::All, Some(&roles));

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "assistant");
        assert_eq!(messages[0].text, "answer");
        assert!(!has_more);
    }
}
