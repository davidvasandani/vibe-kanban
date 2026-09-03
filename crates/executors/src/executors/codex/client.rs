use std::{
    collections::{HashMap, VecDeque},
    io,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use async_trait::async_trait;
use codex_app_server_protocol::{
    ClientInfo, ClientNotification, ClientRequest, CommandExecutionApprovalDecision,
    CommandExecutionRequestApprovalResponse, ConfigBatchWriteParams, ConfigEdit, ConfigReadParams,
    ConfigReadResponse, ConfigWriteResponse, CurrentTimeReadResponse,
    DynamicToolCallOutputContentItem, DynamicToolCallResponse, FileChangeApprovalDecision,
    FileChangeRequestApprovalResponse, GetAccountParams, GetAccountRateLimitsResponse,
    GetAccountResponse, InitializeCapabilities, InitializeParams, InitializeResponse,
    ItemCompletedNotification, JSONRPCError, JSONRPCNotification, JSONRPCRequest, JSONRPCResponse,
    ListMcpServerStatusParams, ListMcpServerStatusResponse, McpAuthStatus,
    McpServerRefreshResponse, McpServerStatusDetail, RequestId, ReviewStartParams,
    ReviewStartResponse, ReviewTarget, ServerRequest, ThreadCompactStartParams,
    ThreadCompactStartResponse, ThreadForkParams, ThreadForkResponse, ThreadItem, ThreadReadParams,
    ThreadReadResponse, ThreadStartParams, ThreadStartResponse, ToolRequestUserInputAnswer,
    ToolRequestUserInputQuestion, ToolRequestUserInputResponse, TurnCompletedNotification,
    TurnStartParams, TurnStartResponse, TurnStatus, UserInput,
};
use codex_protocol::config_types::{CollaborationMode, ModeKind, Settings};
use futures::TryFutureExt;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{self, Value};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncWrite, AsyncWriteExt, BufWriter},
    sync::Mutex,
};
use tokio_util::sync::CancellationToken;
use workspace_utils::approvals::{ApprovalStatus, QuestionStatus};

use super::jsonrpc::{JsonRpcCallbacks, JsonRpcPeer};
use crate::{
    approvals::{ExecutorApprovalError, ExecutorApprovalService},
    env::RepoContext,
    executors::{ExecutorError, codex::normalize_logs::Approval},
    mcp_refresh::{
        McpRefreshControl, McpRefreshErrorCategory, McpServerRefreshSnapshot,
        McpServerRefreshStatus, safe_executor_error,
    },
};

struct PendingPlan {
    item_id: String,
}

pub struct AppServerClient {
    rpc: OnceLock<JsonRpcPeer>,
    log_writer: LogWriter,
    approvals: Option<Arc<dyn ExecutorApprovalService>>,
    thread_id: Mutex<Option<String>>,
    pending_feedback: Mutex<VecDeque<String>>,
    auto_approve: bool,
    plan_mode: bool,
    resolved_model: OnceLock<String>,
    pending_plan: Mutex<Option<PendingPlan>>,
    repo_context: RepoContext,
    commit_reminder: bool,
    commit_reminder_prompt: String,
    commit_reminder_sent: AtomicBool,
    cancel: CancellationToken,
}

impl AppServerClient {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        log_writer: LogWriter,
        approvals: Option<Arc<dyn ExecutorApprovalService>>,
        auto_approve: bool,
        plan_mode: bool,
        repo_context: RepoContext,
        commit_reminder: bool,
        commit_reminder_prompt: String,
        cancel: CancellationToken,
    ) -> Arc<Self> {
        Arc::new(Self {
            rpc: OnceLock::new(),
            log_writer,
            approvals,
            auto_approve,
            plan_mode,
            resolved_model: OnceLock::new(),
            pending_plan: Mutex::new(None),
            thread_id: Mutex::new(None),
            pending_feedback: Mutex::new(VecDeque::new()),
            repo_context,
            commit_reminder,
            commit_reminder_prompt,
            commit_reminder_sent: AtomicBool::new(false),
            cancel,
        })
    }

    pub fn connect(&self, peer: JsonRpcPeer) {
        let _ = self.rpc.set(peer);
    }

    pub fn set_resolved_model(&self, model: String) {
        let _ = self.resolved_model.set(model);
    }

    fn rpc(&self) -> &JsonRpcPeer {
        self.rpc.get().expect("Codex RPC peer not attached")
    }

    pub fn log_writer(&self) -> &LogWriter {
        &self.log_writer
    }

    pub async fn initialize(&self) -> Result<(), ExecutorError> {
        let request = ClientRequest::Initialize {
            request_id: self.next_request_id(),
            params: InitializeParams {
                client_info: ClientInfo {
                    name: "vibe-codex-executor".to_string(),
                    title: None,
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
                capabilities: Some(InitializeCapabilities {
                    experimental_api: true,
                    ..Default::default()
                }),
            },
        };

        self.send_request::<InitializeResponse>(request, "initialize")
            .await?;
        self.send_message(&ClientNotification::Initialized).await
    }

    pub async fn thread_start(
        &self,
        params: ThreadStartParams,
    ) -> Result<ThreadStartResponse, ExecutorError> {
        let request = ClientRequest::ThreadStart {
            request_id: self.next_request_id(),
            params,
        };
        self.send_request(request, "thread/start").await
    }

    pub async fn thread_fork(
        &self,
        params: ThreadForkParams,
    ) -> Result<ThreadForkResponse, ExecutorError> {
        let request = ClientRequest::ThreadFork {
            request_id: self.next_request_id(),
            params,
        };
        self.send_request(request, "thread/fork").await
    }

    pub async fn turn_start_with_mode(
        &self,
        thread_id: String,
        input: Vec<UserInput>,
        collaboration_mode: Option<CollaborationMode>,
    ) -> Result<TurnStartResponse, ExecutorError> {
        let request = ClientRequest::TurnStart {
            request_id: self.next_request_id(),
            params: TurnStartParams {
                thread_id,
                input,
                collaboration_mode,
                ..Default::default()
            },
        };
        self.send_request(request, "turn/start").await
    }

    fn collaboration_mode(&self, mode: ModeKind) -> Result<CollaborationMode, ExecutorError> {
        let model = self.resolved_model.get().cloned().ok_or_else(|| {
            tracing::error!("collaboration_mode called before resolved_model was set");
            ExecutorError::Io(io::Error::other(
                "resolved model not available for collaboration mode",
            ))
        })?;
        Ok(CollaborationMode {
            mode,
            settings: Settings {
                model,
                reasoning_effort: None,
                developer_instructions: None,
            },
        })
    }

    pub fn initial_collaboration_mode(&self) -> Result<CollaborationMode, ExecutorError> {
        if self.plan_mode {
            self.collaboration_mode(ModeKind::Plan)
        } else {
            self.collaboration_mode(ModeKind::Default)
        }
    }

    /// Read the current account.
    ///
    /// When `refresh` is `true`, Codex performs its *guarded* token refresh
    /// (reload `auth.json`; skip the network refresh if another actor already
    /// rotated the token). We only pass `true` from the serialized pre-turn
    /// refresh path (see `codex::auth_refresh`); the plain account check passes
    /// `false` to preserve existing behavior.
    pub async fn get_account(&self, refresh: bool) -> Result<GetAccountResponse, ExecutorError> {
        let request = ClientRequest::GetAccount {
            request_id: self.next_request_id(),
            params: GetAccountParams {
                refresh_token: refresh,
            },
        };
        self.send_request(request, "account/read").await
    }

    pub async fn start_review(
        &self,
        thread_id: String,
        target: ReviewTarget,
    ) -> Result<ReviewStartResponse, ExecutorError> {
        let request = ClientRequest::ReviewStart {
            request_id: self.next_request_id(),
            params: ReviewStartParams {
                thread_id,
                target,
                delivery: None,
            },
        };
        self.send_request(request, "reviewStart").await
    }

    pub async fn list_mcp_server_status(
        &self,
        cursor: Option<String>,
    ) -> Result<ListMcpServerStatusResponse, ExecutorError> {
        let thread_id = self.thread_id.lock().await.clone();
        let request = ClientRequest::McpServerStatusList {
            request_id: self.next_request_id(),
            params: ListMcpServerStatusParams {
                cursor,
                limit: None,
                detail: Some(McpServerStatusDetail::Full),
                thread_id,
            },
        };
        self.send_request(request, "mcpServerStatus/list").await
    }

    pub async fn refresh_mcp_servers(&self) -> Result<(), ExecutorError> {
        let request = ClientRequest::McpServerRefresh {
            request_id: self.next_request_id(),
            params: None,
        };
        self.send_request::<McpServerRefreshResponse>(request, "config/mcpServer/reload")
            .await?;
        Ok(())
    }

    async fn all_mcp_server_statuses(
        &self,
    ) -> Result<Vec<codex_app_server_protocol::McpServerStatus>, ExecutorError> {
        let mut cursor = None;
        let mut servers = Vec::new();
        loop {
            let response = self.list_mcp_server_status(cursor).await?;
            servers.extend(response.data);
            match response.next_cursor {
                Some(next) => cursor = Some(next),
                None => return Ok(servers),
            }
        }
    }

    pub async fn thread_compact_start(
        &self,
        thread_id: String,
    ) -> Result<ThreadCompactStartResponse, ExecutorError> {
        let request = ClientRequest::ThreadCompactStart {
            request_id: self.next_request_id(),
            params: ThreadCompactStartParams { thread_id },
        };
        self.send_request(request, "thread/compact/start").await
    }

    pub async fn thread_read(
        &self,
        thread_id: String,
    ) -> Result<ThreadReadResponse, ExecutorError> {
        let request = ClientRequest::ThreadRead {
            request_id: self.next_request_id(),
            params: ThreadReadParams {
                thread_id,
                include_turns: false,
            },
        };
        self.send_request(request, "thread/read").await
    }

    pub async fn config_batch_write(
        &self,
        edits: Vec<ConfigEdit>,
    ) -> Result<ConfigWriteResponse, ExecutorError> {
        let request = ClientRequest::ConfigBatchWrite {
            request_id: self.next_request_id(),
            params: ConfigBatchWriteParams {
                edits,
                file_path: None,
                expected_version: None,
                reload_user_config: false,
            },
        };
        self.send_request(request, "config/batchWrite").await
    }

    pub async fn config_read(
        &self,
        cwd: Option<String>,
    ) -> Result<ConfigReadResponse, ExecutorError> {
        let request = ClientRequest::ConfigRead {
            request_id: self.next_request_id(),
            params: ConfigReadParams {
                include_layers: false,
                cwd,
            },
        };
        self.send_request(request, "config/read").await
    }

    pub async fn get_account_rate_limits(
        &self,
    ) -> Result<GetAccountRateLimitsResponse, ExecutorError> {
        let request = ClientRequest::GetAccountRateLimits {
            request_id: self.next_request_id(),
            params: None,
        };
        self.send_request(request, "account/rateLimits/read").await
    }

    async fn handle_server_request(
        &self,
        peer: &JsonRpcPeer,
        request: ServerRequest,
    ) -> Result<(), ExecutorError> {
        match request {
            ServerRequest::FileChangeRequestApproval { request_id, params } => {
                let call_id = params.item_id.clone();
                let status = self
                    .request_tool_approval("edit", "codex.apply_patch", &call_id)
                    .await
                    .inspect_err(|err| {
                        if !matches!(
                            err,
                            ExecutorError::ExecutorApprovalError(ExecutorApprovalError::Cancelled)
                        ) {
                            tracing::error!(
                                "Codex file_change approval failed for item_id={}: {err}",
                                call_id
                            );
                        }
                    })?;
                self.log_writer
                    .log_raw(
                        &Approval::approval_response(
                            call_id,
                            "codex.apply_patch".to_string(),
                            status.clone(),
                        )
                        .raw(),
                    )
                    .await?;
                let (decision, feedback) = self.file_change_decision(&status);
                let response = FileChangeRequestApprovalResponse { decision };
                send_server_response(peer, request_id, response).await?;
                if let Some(message) = feedback {
                    tracing::debug!("queueing file change denial feedback: {message}");
                    self.enqueue_feedback(message).await;
                }
                Ok(())
            }
            ServerRequest::CommandExecutionRequestApproval { request_id, params } => {
                let call_id = params.item_id.clone();
                let status = self
                    .request_tool_approval("bash", "codex.exec_command", &call_id)
                    .await
                    .inspect_err(|err| {
                        if !matches!(
                            err,
                            ExecutorError::ExecutorApprovalError(ExecutorApprovalError::Cancelled)
                        ) {
                            tracing::error!(
                                "Codex command_execution approval failed for item_id={}: {err}",
                                call_id
                            );
                        }
                    })?;
                self.log_writer
                    .log_raw(
                        &Approval::approval_response(
                            call_id,
                            "codex.exec_command".to_string(),
                            status.clone(),
                        )
                        .raw(),
                    )
                    .await?;
                let (decision, feedback) = self.command_execution_decision(&status);
                let response = CommandExecutionRequestApprovalResponse { decision };
                send_server_response(peer, request_id, response).await?;
                if let Some(message) = feedback {
                    tracing::debug!("queueing exec denial feedback: {message}");
                    self.enqueue_feedback(message).await;
                }
                Ok(())
            }
            ServerRequest::ToolRequestUserInput { request_id, params } => {
                let call_id = params.item_id.clone();
                let question_count = params.questions.len();
                let status = self
                    .request_question_answer(question_count, &call_id)
                    .await
                    .inspect_err(|err| {
                        if !matches!(
                            err,
                            ExecutorError::ExecutorApprovalError(ExecutorApprovalError::Cancelled)
                        ) {
                            tracing::error!(
                                "Codex question approval failed for call_id={}: {err}",
                                call_id
                            );
                        }
                    })?;
                self.log_writer
                    .log_raw(&Approval::question_response(call_id.clone(), status.clone()).raw())
                    .await?;
                let response = match &status {
                    QuestionStatus::Answered { answers } => {
                        let answers_map: HashMap<String, Vec<String>> = answers
                            .iter()
                            .map(|qa| (qa.question.clone(), qa.answer.clone()))
                            .collect();
                        answers_to_codex_format(&params.questions, &answers_map)
                    }
                    _ => ToolRequestUserInputResponse {
                        answers: HashMap::new(),
                    },
                };
                send_server_response(peer, request_id, response).await?;
                Ok(())
            }
            ServerRequest::DynamicToolCall { request_id, params } => {
                tracing::warn!(
                    "received unsupported dynamic tool call: tool={} call_id={}",
                    params.tool,
                    params.call_id
                );
                let response = DynamicToolCallResponse {
                    content_items: vec![DynamicToolCallOutputContentItem::InputText {
                        text: format!(
                            "Dynamic tool '{}' is not supported by this client.",
                            params.tool
                        ),
                    }],
                    success: false,
                };
                send_server_response(peer, request_id, response).await?;
                Ok(())
            }
            ServerRequest::CurrentTimeRead { request_id, .. } => {
                let current_time_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or_default();
                let response = CurrentTimeReadResponse { current_time_at };
                send_server_response(peer, request_id, response).await?;
                Ok(())
            }
            ServerRequest::ChatgptAuthTokensRefresh { .. }
            | ServerRequest::McpServerElicitationRequest { .. }
            | ServerRequest::AttestationGenerate { .. }
            | ServerRequest::PermissionsRequestApproval { .. } => {
                tracing::warn!("received unhandled v2 server request: {:?}", request);
                let response = JSONRPCResponse {
                    id: request.id().clone(),
                    result: Value::Null,
                };
                peer.send(&response).await
            }
            ServerRequest::ApplyPatchApproval { .. }
            | ServerRequest::ExecCommandApproval { .. } => {
                tracing::error!(
                    "received deprecated v1 server request (session may have been started with legacy API): {:?}",
                    request
                );
                Err(ExecutorApprovalError::RequestFailed(
                    "deprecated v1 server request".to_string(),
                )
                .into())
            }
        }
    }

    async fn request_tool_approval(
        &self,
        tool_name: &str,
        display_tool_name: &str,
        tool_call_id: &str,
    ) -> Result<ApprovalStatus, ExecutorError> {
        if self.auto_approve {
            return Ok(ApprovalStatus::Approved);
        }
        let approval_service = self
            .approvals
            .as_ref()
            .ok_or(ExecutorApprovalError::ServiceUnavailable)?;

        let approval_id = approval_service
            .create_tool_approval(tool_name)
            .or_else(|err| async {
                self.handle_approval_error(display_tool_name, tool_call_id)
                    .await;
                Err(err)
            })
            .await?;

        let _ = self
            .log_writer
            .log_raw(
                &Approval::approval_requested(
                    tool_call_id.to_string(),
                    display_tool_name.to_string(),
                    approval_id.clone(),
                )
                .raw(),
            )
            .await;

        approval_service
            .wait_tool_approval(&approval_id, self.cancel.clone())
            .or_else(|err| async {
                self.handle_approval_error(display_tool_name, tool_call_id)
                    .await;
                Err(err)
            })
            .await
            .map_err(ExecutorError::from)
    }

    async fn handle_approval_error(&self, display_tool_name: &str, tool_call_id: &str) {
        let _ = self
            .log_writer
            .log_raw(
                &Approval::approval_response(
                    tool_call_id.to_string(),
                    display_tool_name.to_string(),
                    ApprovalStatus::TimedOut,
                )
                .raw(),
            )
            .await;
    }

    async fn request_question_answer(
        &self,
        question_count: usize,
        tool_call_id: &str,
    ) -> Result<QuestionStatus, ExecutorError> {
        let approval_service = self
            .approvals
            .as_ref()
            .ok_or(ExecutorApprovalError::ServiceUnavailable)?;

        let approval_id = approval_service
            .create_question_approval("question", question_count)
            .or_else(|err| async {
                self.handle_question_error(tool_call_id).await;
                Err(err)
            })
            .await?;

        let _ = self
            .log_writer
            .log_raw(
                &Approval::approval_requested(
                    tool_call_id.to_string(),
                    "codex.question".to_string(),
                    approval_id.clone(),
                )
                .raw(),
            )
            .await;

        approval_service
            .wait_question_answer(&approval_id, self.cancel.clone())
            .or_else(|err| async {
                self.handle_question_error(tool_call_id).await;
                Err(err)
            })
            .await
            .map_err(ExecutorError::from)
    }

    async fn handle_question_error(&self, tool_call_id: &str) {
        let _ = self
            .log_writer
            .log_raw(
                &Approval::question_response(tool_call_id.to_string(), QuestionStatus::TimedOut)
                    .raw(),
            )
            .await;
    }

    async fn handle_plan_completed(&self, plan: PendingPlan) -> Result<bool, ExecutorError> {
        let approval_service = self
            .approvals
            .as_ref()
            .ok_or(ExecutorApprovalError::ServiceUnavailable)?;

        let approval_id = approval_service
            .create_tool_approval("plan")
            .or_else(|err| async {
                self.handle_approval_error("codex.plan", &plan.item_id)
                    .await;
                Err(err)
            })
            .await?;

        let _ = self
            .log_writer
            .log_raw(
                &Approval::approval_requested(
                    plan.item_id.clone(),
                    "codex.plan".to_string(),
                    approval_id.clone(),
                )
                .raw(),
            )
            .await;

        let status = approval_service
            .wait_tool_approval(&approval_id, self.cancel.clone())
            .or_else(|err| async {
                self.handle_approval_error("codex.plan", &plan.item_id)
                    .await;
                Err(err)
            })
            .await
            .map_err(ExecutorError::from)?;

        self.log_writer
            .log_raw(
                &Approval::approval_response(
                    plan.item_id,
                    "codex.plan".to_string(),
                    status.clone(),
                )
                .raw(),
            )
            .await?;

        let Some(thread_id) = self.thread_id.lock().await.clone() else {
            return Ok(true);
        };

        match status {
            ApprovalStatus::Approved => {
                self.spawn_turn_start(
                    thread_id,
                    "Implement the plan.".to_string(),
                    Some(self.collaboration_mode(ModeKind::Default)?),
                );
                Ok(false)
            }
            ApprovalStatus::Denied { reason } => {
                let feedback = reason
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());
                if let Some(feedback_text) = feedback {
                    self.spawn_turn_start(
                        thread_id,
                        format!("User feedback on the plan: {feedback_text}"),
                        Some(self.collaboration_mode(ModeKind::Plan)?),
                    );
                    Ok(false)
                } else {
                    Ok(true)
                }
            }
            ApprovalStatus::TimedOut | ApprovalStatus::Pending => Ok(true),
        }
    }

    pub async fn register_session(&self, thread_id: &str) -> Result<(), ExecutorError> {
        {
            let mut guard = self.thread_id.lock().await;
            guard.replace(thread_id.to_string());
        }
        self.flush_pending_feedback().await;
        Ok(())
    }

    async fn send_message<M>(&self, message: &M) -> Result<(), ExecutorError>
    where
        M: Serialize + Sync,
    {
        self.rpc().send(message).await
    }

    async fn send_request<R>(&self, request: ClientRequest, label: &str) -> Result<R, ExecutorError>
    where
        R: DeserializeOwned + std::fmt::Debug,
    {
        let request_id = request_id(&request);
        self.rpc()
            .request(request_id, &request, label, self.cancel.clone())
            .await
    }

    fn next_request_id(&self) -> RequestId {
        self.rpc().next_request_id()
    }

    fn command_execution_decision(
        &self,
        status: &ApprovalStatus,
    ) -> (CommandExecutionApprovalDecision, Option<String>) {
        if self.auto_approve {
            return (CommandExecutionApprovalDecision::AcceptForSession, None);
        }

        match status {
            ApprovalStatus::Approved => (CommandExecutionApprovalDecision::Accept, None),
            ApprovalStatus::Denied { reason } => {
                let feedback = reason
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());
                if feedback.is_some() {
                    (CommandExecutionApprovalDecision::Cancel, feedback)
                } else {
                    (CommandExecutionApprovalDecision::Decline, None)
                }
            }
            ApprovalStatus::TimedOut => (CommandExecutionApprovalDecision::Decline, None),
            ApprovalStatus::Pending => (CommandExecutionApprovalDecision::Decline, None),
        }
    }

    fn file_change_decision(
        &self,
        status: &ApprovalStatus,
    ) -> (FileChangeApprovalDecision, Option<String>) {
        if self.auto_approve {
            return (FileChangeApprovalDecision::AcceptForSession, None);
        }

        match status {
            ApprovalStatus::Approved => (FileChangeApprovalDecision::Accept, None),
            ApprovalStatus::Denied { reason } => {
                let feedback = reason
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());
                if feedback.is_some() {
                    (FileChangeApprovalDecision::Cancel, feedback)
                } else {
                    (FileChangeApprovalDecision::Decline, None)
                }
            }
            ApprovalStatus::TimedOut => (FileChangeApprovalDecision::Decline, None),
            ApprovalStatus::Pending => (FileChangeApprovalDecision::Decline, None),
        }
    }

    async fn enqueue_feedback(&self, message: String) {
        if message.trim().is_empty() {
            return;
        }
        let mut guard = self.pending_feedback.lock().await;
        guard.push_back(message);
    }

    /// Sends pending feedback messages as new turns.
    /// Returns `true` if any messages were sent.
    async fn flush_pending_feedback(&self) -> bool {
        let messages: Vec<String> = {
            let mut guard = self.pending_feedback.lock().await;
            guard.drain(..).collect()
        };

        if messages.is_empty() {
            return false;
        }

        let Some(thread_id) = self.thread_id.lock().await.clone() else {
            tracing::warn!(
                "pending Codex feedback but thread id unavailable; dropping {} messages",
                messages.len()
            );
            return false;
        };

        let mut sent = false;
        for message in messages {
            let trimmed = message.trim();
            if trimmed.is_empty() {
                continue;
            }
            self.spawn_user_message(thread_id.clone(), format!("User feedback: {trimmed}"));
            sent = true;
        }
        sent
    }

    fn spawn_turn_start(
        &self,
        thread_id: String,
        message: String,
        collaboration_mode: Option<CollaborationMode>,
    ) {
        let peer = self.rpc().clone();
        let cancel = self.cancel.clone();
        let request = ClientRequest::TurnStart {
            request_id: peer.next_request_id(),
            params: TurnStartParams {
                thread_id,
                input: vec![UserInput::Text {
                    text: message,
                    text_elements: vec![],
                }],
                collaboration_mode,
                ..Default::default()
            },
        };
        tokio::spawn(async move {
            if let Err(err) = peer
                .request::<TurnStartResponse, _>(
                    request_id(&request),
                    &request,
                    "turn/start",
                    cancel,
                )
                .await
            {
                tracing::error!("failed to send user message: {err}");
            }
        });
    }

    fn spawn_user_message(&self, thread_id: String, message: String) {
        self.spawn_turn_start(thread_id, message, None);
    }
}

#[async_trait]
impl McpRefreshControl for AppServerClient {
    async fn queue_refresh(&self) -> Result<(), McpRefreshErrorCategory> {
        tokio::time::timeout(
            std::time::Duration::from_secs(30),
            self.refresh_mcp_servers(),
        )
        .await
        .map_err(|_| McpRefreshErrorCategory::Timeout)?
        .map_err(|_| McpRefreshErrorCategory::Internal)
    }

    async fn list_servers(&self) -> Result<Vec<McpServerRefreshSnapshot>, McpRefreshErrorCategory> {
        let servers = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            self.all_mcp_server_statuses(),
        )
        .await
        .map_err(|_| McpRefreshErrorCategory::Timeout)?
        .map_err(|_| McpRefreshErrorCategory::CapabilityListFailed)?;
        Ok(servers
            .into_iter()
            .map(|server| {
                let auth_failed = matches!(server.auth_status, McpAuthStatus::NotLoggedIn);
                let (tool_names, tool_schema_fingerprint) = tool_inventory_evidence(&server.tools);
                McpServerRefreshSnapshot {
                    server_id: server.name,
                    status: if auth_failed {
                        McpServerRefreshStatus::FailedUnavailable
                    } else {
                        McpServerRefreshStatus::Ready
                    },
                    tool_count: Some(server.tools.len() as u32),
                    tool_names: Some(tool_names),
                    tool_schema_fingerprint: Some(tool_schema_fingerprint),
                    resource_count: Some(
                        (server.resources.len() + server.resource_templates.len()) as u32,
                    ),
                    prompt_count: None,
                    // The pinned status protocol does not expose reuse/restart.
                    restart_occurred: None,
                    error: auth_failed.then(|| {
                        safe_executor_error(McpRefreshErrorCategory::AuthenticationFailed)
                    }),
                }
            })
            .collect())
    }
}

/// Produce bounded evidence for the exact tool generation Codex reports for a
/// loaded thread. Reload acknowledgement alone does not prove adoption.
fn tool_inventory_evidence(
    tools: &HashMap<String, codex_protocol::mcp::Tool>,
) -> (Vec<String>, String) {
    let mut tool_names: Vec<_> = tools.keys().cloned().collect();
    tool_names.sort();

    let schemas: Vec<_> = tool_names
        .iter()
        .map(|key| {
            let tool = &tools[key];
            serde_json::json!({
                "key": key,
                "name": tool.name,
                "input_schema": tool.input_schema,
                "output_schema": tool.output_schema,
            })
        })
        .collect();
    let encoded = serde_json::to_vec(&schemas)
        .expect("Codex MCP tool schemas are JSON-serializable protocol values");
    let digest = Sha256::digest(encoded);
    (tool_names, format!("{digest:x}"))
}

#[async_trait]
impl JsonRpcCallbacks for AppServerClient {
    async fn on_request(
        &self,
        peer: &JsonRpcPeer,
        raw: &str,
        request: JSONRPCRequest,
    ) -> Result<(), ExecutorError> {
        self.log_writer.log_raw(raw).await?;
        match ServerRequest::try_from(request.clone()) {
            Ok(server_request) => self.handle_server_request(peer, server_request).await,
            Err(err) => {
                tracing::debug!("Unhandled server request `{}`: {err}", request.method);
                let response = JSONRPCResponse {
                    id: request.id,
                    result: Value::Null,
                };
                peer.send(&response).await
            }
        }
    }

    async fn on_response(
        &self,
        _peer: &JsonRpcPeer,
        raw: &str,
        _response: &JSONRPCResponse,
    ) -> Result<(), ExecutorError> {
        self.log_writer.log_raw(raw).await
    }

    async fn on_error(
        &self,
        _peer: &JsonRpcPeer,
        raw: &str,
        _error: &JSONRPCError,
    ) -> Result<(), ExecutorError> {
        self.log_writer.log_raw(raw).await
    }

    async fn on_notification(
        &self,
        _peer: &JsonRpcPeer,
        raw: &str,
        notification: JSONRPCNotification,
    ) -> Result<bool, ExecutorError> {
        self.log_writer.log_raw(raw).await?;

        let method = notification.method.as_str();

        // Detect completed plan items in the notification stream
        if self.plan_mode
            && method == "item/completed"
            && let Some(ref params) = notification.params
            && let Ok(completed) =
                serde_json::from_value::<ItemCompletedNotification>(params.clone())
            && let ThreadItem::Plan { id, .. } = completed.item
        {
            *self.pending_plan.lock().await = Some(PendingPlan { item_id: id });
        }

        // V2 turn completion detection
        if method == "turn/completed" {
            let mut keep_alive = false;

            if let Some(params) = notification.params
                && let Ok(completed) = serde_json::from_value::<TurnCompletedNotification>(params)
                && completed.turn.status == TurnStatus::Interrupted
            {
                tracing::debug!("codex turn interrupted; flushing feedback queue");
                if self.flush_pending_feedback().await {
                    keep_alive = true;
                }
            }

            // Handle plan approval on turn completion
            let pending = if self.plan_mode {
                self.pending_plan.lock().await.take()
            } else {
                None
            };
            if let Some(plan) = pending {
                return self.handle_plan_completed(plan).await;
            }

            // Handle commit reminder on turn completion
            if !keep_alive
                && self.commit_reminder
                && !self.commit_reminder_sent.swap(true, Ordering::SeqCst)
                && let status = self.repo_context.check_uncommitted_changes().await
                && !status.is_empty()
                && let Some(thread_id) = self.thread_id.lock().await.clone()
            {
                let prompt = format!("{}\n{}", self.commit_reminder_prompt, status);
                self.spawn_user_message(thread_id, prompt);
                return Ok(false);
            }

            return Ok(!keep_alive);
        }

        Ok(false)
    }

    async fn on_non_json(&self, raw: &str) -> Result<(), ExecutorError> {
        self.log_writer.log_raw(raw).await?;
        Ok(())
    }
}

async fn send_server_response<T>(
    peer: &JsonRpcPeer,
    request_id: RequestId,
    response: T,
) -> Result<(), ExecutorError>
where
    T: Serialize,
{
    let payload = JSONRPCResponse {
        id: request_id,
        result: serde_json::to_value(response)
            .map_err(|err| ExecutorError::Io(io::Error::other(err.to_string())))?,
    };

    peer.send(&payload).await
}

/// Convert our `HashMap<question_text, Vec<answer_labels>>` answer format to
/// Codex's `HashMap<question_id, ToolRequestUserInputAnswer>` format.
fn answers_to_codex_format(
    questions: &[ToolRequestUserInputQuestion],
    answers: &HashMap<String, Vec<String>>,
) -> ToolRequestUserInputResponse {
    let codex_answers = questions
        .iter()
        .filter_map(|q| {
            answers.get(&q.question).map(|answer_vec| {
                (
                    q.id.clone(),
                    ToolRequestUserInputAnswer {
                        answers: answer_vec.clone(),
                    },
                )
            })
        })
        .collect();

    ToolRequestUserInputResponse {
        answers: codex_answers,
    }
}

fn request_id(request: &ClientRequest) -> RequestId {
    match request {
        ClientRequest::Initialize { request_id, .. }
        | ClientRequest::ThreadStart { request_id, .. }
        | ClientRequest::ThreadFork { request_id, .. }
        | ClientRequest::TurnStart { request_id, .. }
        | ClientRequest::GetAccount { request_id, .. }
        | ClientRequest::ReviewStart { request_id, .. }
        | ClientRequest::McpServerRefresh { request_id, .. }
        | ClientRequest::McpServerStatusList { request_id, .. }
        | ClientRequest::ThreadCompactStart { request_id, .. }
        | ClientRequest::ThreadRead { request_id, .. }
        | ClientRequest::ConfigRead { request_id, .. }
        | ClientRequest::ConfigBatchWrite { request_id, .. }
        | ClientRequest::GetAccountRateLimits { request_id, .. } => request_id.clone(),
        _ => unreachable!("request_id called for unsupported request variant"),
    }
}

#[derive(Clone)]
pub struct LogWriter {
    writer: Arc<Mutex<BufWriter<Box<dyn AsyncWrite + Send + Unpin>>>>,
}

impl LogWriter {
    pub fn new(writer: impl AsyncWrite + Send + Unpin + 'static) -> Self {
        Self {
            writer: Arc::new(Mutex::new(BufWriter::new(Box::new(writer)))),
        }
    }

    pub async fn log_raw(&self, raw: &str) -> Result<(), ExecutorError> {
        let mut guard = self.writer.lock().await;
        guard
            .write_all(raw.as_bytes())
            .await
            .map_err(ExecutorError::Io)?;
        guard.write_all(b"\n").await.map_err(ExecutorError::Io)?;
        guard.flush().await.map_err(ExecutorError::Io)?;
        Ok(())
    }
}

#[cfg(test)]
mod mcp_inventory_tests {
    use codex_protocol::mcp::Tool;

    use super::*;

    fn tool(name: &str, property_type: &str) -> Tool {
        Tool {
            name: name.to_string(),
            title: None,
            description: None,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {"value": {"type": property_type}}
            }),
            output_schema: None,
            annotations: None,
            icons: None,
            meta: None,
        }
    }

    #[test]
    fn evidence_detects_stdio_add_remove_and_schema_change() {
        let original = HashMap::from([(
            "sn_access_cycle_report".to_string(),
            tool("sn_access_cycle_report", "string"),
        )]);
        let added = HashMap::from([
            (
                "sn_access_cycle_report".to_string(),
                tool("sn_access_cycle_report", "string"),
            ),
            (
                "entra_user_lookup".to_string(),
                tool("entra_user_lookup", "string"),
            ),
        ]);
        let removed = HashMap::from([(
            "entra_user_lookup".to_string(),
            tool("entra_user_lookup", "string"),
        )]);
        let schema_changed = HashMap::from([(
            "entra_user_lookup".to_string(),
            tool("entra_user_lookup", "integer"),
        )]);

        let original_evidence = tool_inventory_evidence(&original);
        let added_evidence = tool_inventory_evidence(&added);
        let removed_evidence = tool_inventory_evidence(&removed);
        let changed_evidence = tool_inventory_evidence(&schema_changed);

        assert_eq!(
            added_evidence.0,
            vec!["entra_user_lookup", "sn_access_cycle_report"]
        );
        assert_eq!(removed_evidence.0, vec!["entra_user_lookup"]);
        assert_ne!(original_evidence.1, added_evidence.1);
        assert_ne!(added_evidence.1, removed_evidence.1);
        assert_ne!(removed_evidence.1, changed_evidence.1);
        assert_eq!(removed_evidence.0, changed_evidence.0);
    }

    #[test]
    fn evidence_is_stable_across_hash_map_order() {
        let first = HashMap::from([
            ("b".to_string(), tool("b", "integer")),
            ("a".to_string(), tool("a", "string")),
        ]);
        let second = HashMap::from([
            ("a".to_string(), tool("a", "string")),
            ("b".to_string(), tool("b", "integer")),
        ]);

        assert_eq!(
            tool_inventory_evidence(&first),
            tool_inventory_evidence(&second)
        );
    }
}
