use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    time::Duration,
};

use axum::{
    Extension, Json, Router, extract::State, middleware::from_fn_with_state,
    response::Json as ResponseJson, routing::get,
};
use db::models::{
    execution_process::{ExecutionProcess, ExecutionProcessRunReason},
    scratch::DraftFollowUpData,
    session::Session,
};
use deployment::Deployment;
use executors::{
    actions::ExecutorActionType,
    mcp_recovery::{McpRecoveryResult, McpRecoveryScope, McpRecoveryStatus, McpRestartDisposition},
    profile::ExecutorConfig,
};
use serde::{Deserialize, Serialize};
use services::services::{
    container::ContainerService,
    queued_message::{QueueStatus, QueuedMessageService},
};
use ts_rs::TS;
use utils::response::ApiResponse;

use crate::{DeploymentImpl, error::ApiError, middleware::load_session_middleware};

/// Request body for queueing a follow-up message
#[derive(Debug, Deserialize, TS)]
struct QueueMessageRequest {
    pub message: String,
    pub executor_config: ExecutorConfig,
}

#[derive(Debug, Deserialize, TS)]
struct QueueMcpRestartRequest {
    pub message: String,
    pub executor_config: ExecutorConfig,
    pub confirmed_running_restart: bool,
}

#[derive(Debug, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum QueueMcpRestartResult {
    ConfirmationRequired,
    Queued,
    Started,
}

static ACTIVE_MCP_SESSION_RESTARTS: LazyLock<Mutex<HashMap<uuid::Uuid, uuid::Uuid>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

struct ActiveSessionRestartGuard {
    session_id: uuid::Uuid,
    token: uuid::Uuid,
    active: bool,
}

impl ActiveSessionRestartGuard {
    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for ActiveSessionRestartGuard {
    fn drop(&mut self) {
        if self.active {
            let mut active = ACTIVE_MCP_SESSION_RESTARTS
                .lock()
                .expect("MCP session restart lock poisoned");
            if active.get(&self.session_id) == Some(&self.token) {
                active.remove(&self.session_id);
            }
        }
    }
}

pub async fn supersede_mcp_session_restart(session_id: uuid::Uuid, deployment: &DeploymentImpl) {
    ACTIVE_MCP_SESSION_RESTARTS
        .lock()
        .expect("MCP session restart lock poisoned")
        .remove(&session_id);
    if deployment
        .queued_message_service()
        .has_mcp_restart(session_id)
    {
        deployment
            .queued_message_service()
            .supersede_mcp_restart(session_id);
    }
    deployment
        .container()
        .clear_mcp_restart_tracking(session_id)
        .await;
}

struct RestartReservationGuard {
    service: QueuedMessageService,
    session_id: uuid::Uuid,
    reservation: uuid::Uuid,
    active: bool,
}

impl RestartReservationGuard {
    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for RestartReservationGuard {
    fn drop(&mut self) {
        if self.active {
            self.service
                .cancel_mcp_restart(self.session_id, self.reservation);
        }
    }
}

async fn queue_mcp_restart_impl(
    session: &Session,
    deployment: &DeploymentImpl,
    payload: QueueMcpRestartRequest,
    prepare_tracking: bool,
) -> Result<QueueMcpRestartResult, ApiError> {
    let was_running =
        db::models::execution_process::ExecutionProcess::has_running_coding_agent_for_session(
            &deployment.db().pool,
            session.id,
        )
        .await?;
    if was_running && !payload.confirmed_running_restart {
        return Ok(QueueMcpRestartResult::ConfirmationRequired);
    }

    let data = DraftFollowUpData {
        message: payload.message,
        executor_config: payload.executor_config,
    };
    // A reservation is deliberately invisible to finalization until the second
    // authoritative running-state check completes.
    let reservation = deployment
        .queued_message_service()
        .reserve_mcp_restart(session.id, data);
    let mut reservation_guard = RestartReservationGuard {
        service: deployment.queued_message_service().clone(),
        session_id: session.id,
        reservation,
        active: true,
    };

    let running_result =
        db::models::execution_process::ExecutionProcess::has_running_coding_agent_for_session(
            &deployment.db().pool,
            session.id,
        )
        .await;
    let running = match running_result {
        Ok(running) => running,
        Err(error) => {
            deployment
                .queued_message_service()
                .cancel_mcp_restart(session.id, reservation);
            reservation_guard.disarm();
            return Err(error.into());
        }
    };
    if running && !payload.confirmed_running_restart {
        return Ok(QueueMcpRestartResult::ConfirmationRequired);
    }
    if prepare_tracking {
        deployment
            .container()
            .prepare_mcp_restart(session.workspace_id, session.id)
            .await?;
    }
    let queued = if running {
        let queued_at = deployment
            .queued_message_service()
            .commit_mcp_restart(session.id, reservation);
        reservation_guard.disarm();
        let still_running_result =
            db::models::execution_process::ExecutionProcess::has_running_coding_agent_for_session(
                &deployment.db().pool,
                session.id,
            )
            .await;
        if matches!(still_running_result, Ok(true)) {
            return Ok(QueueMcpRestartResult::Queued);
        }
        if let Err(error) = still_running_result {
            tracing::warn!(
                session_id = %session.id,
                %error,
                "Could not recheck agent state after committing restart; request handler retains ownership"
            );
        }
        queued_at.and_then(|queued_at| {
            deployment
                .queued_message_service()
                .take_committed_mcp_restart(session.id, queued_at)
        })
    } else {
        let queued = deployment
            .queued_message_service()
            .take_mcp_restart(session.id, reservation);
        reservation_guard.disarm();
        queued
    };

    let result = if let Some(queued) = queued {
        if queued.restart_agent
            && deployment
                .queued_message_service()
                .is_mcp_restart_start_blocked(session.id)
        {
            let deferred_deployment = deployment.clone();
            let deferred_session = session.clone();
            tokio::spawn(async move {
                if !deferred_deployment
                    .queued_message_service()
                    .wait_for_mcp_restart_start(deferred_session.id)
                    .await
                {
                    deferred_deployment
                        .container()
                        .clear_mcp_restart_tracking(deferred_session.id)
                        .await;
                    return;
                }
                deferred_deployment
                    .queued_message_service()
                    .finish_workspace_mcp_restart(deferred_session.id);
                deferred_deployment
                    .container()
                    .reap_warm_process_for_mcp_restart(deferred_session.id)
                    .await;
                if super::follow_up(
                    Extension(deferred_session.clone()),
                    State(deferred_deployment.clone()),
                    Json(super::CreateFollowUpAttempt {
                        prompt: queued.data.message,
                        executor_config: queued.data.executor_config,
                        retry_process_id: None,
                        force_when_dirty: None,
                        perform_git_reset: None,
                    }),
                )
                .await
                .is_err()
                {
                    deferred_deployment
                        .container()
                        .clear_mcp_restart_tracking(deferred_session.id)
                        .await;
                }
            });
            return Ok(QueueMcpRestartResult::Queued);
        }
        deployment
            .queued_message_service()
            .finish_workspace_mcp_restart(session.id);
        deployment
            .container()
            .reap_warm_process_for_mcp_restart(session.id)
            .await;
        let _ = super::follow_up(
            Extension(session.clone()),
            State(deployment.clone()),
            Json(super::CreateFollowUpAttempt {
                prompt: queued.data.message,
                executor_config: queued.data.executor_config,
                retry_process_id: None,
                force_when_dirty: None,
                perform_git_reset: None,
            }),
        )
        .await?;
        QueueMcpRestartResult::Started
    } else {
        // Finalization claimed this exact message and owns starting it.
        QueueMcpRestartResult::Queued
    };
    Ok(result)
}

async fn queue_mcp_restart(
    Extension(session): Extension<Session>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<QueueMcpRestartRequest>,
) -> Result<ResponseJson<ApiResponse<QueueMcpRestartResult>>, ApiError> {
    let result = match queue_mcp_restart_impl(&session, &deployment, payload, true).await {
        Ok(result) => result,
        Err(error) => {
            deployment
                .container()
                .clear_mcp_restart_tracking(session.id)
                .await;
            return Err(error);
        }
    };
    Ok(ResponseJson(ApiResponse::success(result)))
}

fn executor_config(process: &ExecutionProcess) -> Result<ExecutorConfig, String> {
    let action = process.executor_action().map_err(|error| {
        format!(
            "Could not read executor configuration for {}: {error}",
            process.id
        )
    })?;
    match action.typ() {
        ExecutorActionType::CodingAgentInitialRequest(request) => {
            Ok(request.executor_config.clone())
        }
        ExecutorActionType::CodingAgentFollowUpRequest(request) => {
            Ok(request.executor_config.clone())
        }
        _ => Err(format!(
            "Execution {} is not a resumable coding-agent task",
            process.id
        )),
    }
}

pub async fn restart_mcp_session(
    session: &Session,
    deployment: &DeploymentImpl,
) -> Result<McpRecoveryResult, ApiError> {
    let restart_token = uuid::Uuid::new_v4();
    let already_active = {
        let mut active = ACTIVE_MCP_SESSION_RESTARTS
            .lock()
            .expect("MCP session restart lock poisoned");
        if let std::collections::hash_map::Entry::Vacant(entry) = active.entry(session.id) {
            entry.insert(restart_token);
            false
        } else {
            true
        }
    };
    if already_active
        && let Some(current) = deployment
            .container()
            .mcp_refresh_status(session.workspace_id, session.id)
            .await?
    {
        return Ok(McpRecoveryResult {
            generation: current.generation,
            scope: McpRecoveryScope::Session,
            workspace_id: session.workspace_id,
            session_id: Some(session.id),
            status: McpRecoveryStatus::InProgress,
            disposition: McpRestartDisposition::AlreadyInProgress,
            requested_at: current.requested_at,
            completed_at: None,
            executor: session
                .executor
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            servers: current.servers,
            error: current.error,
        });
    }
    if already_active {
        return Err(ApiError::Conflict(
            "An MCP session restart is already in progress".to_string(),
        ));
    }
    let mut active_guard = ActiveSessionRestartGuard {
        session_id: session.id,
        token: restart_token,
        active: true,
    };
    let latest = ExecutionProcess::find_by_session_id(&deployment.db().pool, session.id, false)
        .await?
        .into_iter()
        .rev()
        .find(|process| process.run_reason == ExecutionProcessRunReason::CodingAgent)
        .ok_or_else(|| {
            ApiError::Conflict("Session has no coding-agent execution to restart".into())
        })?;
    let config = executor_config(&latest).map_err(ApiError::Conflict)?;
    let executor = config.executor.to_string();
    let tracking = deployment
        .container()
        .prepare_mcp_restart(session.workspace_id, session.id)
        .await?;
    let result = match queue_mcp_restart_impl(
        session,
        deployment,
        QueueMcpRestartRequest {
            message: "Continue the existing task after restarting the agent process. Re-check MCP availability before relying on MCP tools.".to_string(),
            executor_config: config,
            // Calling the restart endpoint is itself the explicit confirmation.
            confirmed_running_restart: true,
        },
        false,
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            deployment
                .container()
                .clear_mcp_restart_tracking(session.id)
                .await;
            return Err(error);
        }
    };
    let disposition = match result {
        QueueMcpRestartResult::Queued => McpRestartDisposition::Queued,
        QueueMcpRestartResult::Started => McpRestartDisposition::Started,
        QueueMcpRestartResult::ConfirmationRequired => {
            unreachable!("restart endpoint supplies explicit running-turn confirmation")
        }
    };
    let servers = tracking.servers;
    let cleanup_deployment = deployment.clone();
    let cleanup_session = session.clone();
    let cleanup_token = restart_token;
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if cleanup_deployment
                .queued_message_service()
                .has_mcp_restart(cleanup_session.id)
            {
                continue;
            }
            let terminal = cleanup_deployment
                .container()
                .mcp_refresh_status(cleanup_session.workspace_id, cleanup_session.id)
                .await
                .ok()
                .flatten()
                .is_some_and(|state| {
                    !matches!(
                        state.status,
                        executors::mcp_refresh::McpRefreshStatus::PendingNextTurn
                            | executors::mcp_refresh::McpRefreshStatus::Busy
                    )
                });
            if terminal
                || cleanup_deployment
                    .container()
                    .mcp_refresh_status(cleanup_session.workspace_id, cleanup_session.id)
                    .await
                    .ok()
                    .flatten()
                    .is_none()
            {
                break;
            }
        }
        let mut active = ACTIVE_MCP_SESSION_RESTARTS
            .lock()
            .expect("MCP session restart lock poisoned");
        if active.get(&cleanup_session.id) == Some(&cleanup_token) {
            active.remove(&cleanup_session.id);
        }
    });
    active_guard.disarm();
    Ok(McpRecoveryResult {
        generation: tracking.generation,
        scope: McpRecoveryScope::Session,
        workspace_id: session.workspace_id,
        session_id: Some(session.id),
        status: McpRecoveryStatus::Accepted,
        disposition,
        requested_at: tracking.requested_at,
        completed_at: None,
        executor,
        servers,
        error: None,
    })
}

async fn restart_mcp_session_route(
    Extension(session): Extension<Session>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<McpRecoveryResult>>, ApiError> {
    let result = restart_mcp_session(&session, &deployment).await?;
    Ok(ResponseJson(ApiResponse::success(result)))
}

/// Queue a follow-up message to be executed when the current execution finishes
async fn queue_message(
    Extension(session): Extension<Session>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<QueueMessageRequest>,
) -> Result<ResponseJson<ApiResponse<QueueStatus>>, ApiError> {
    let data = DraftFollowUpData {
        message: payload.message,
        executor_config: payload.executor_config,
    };

    let queued = deployment
        .queued_message_service()
        .queue_message(session.id, data);

    deployment
        .track_if_analytics_allowed(
            "follow_up_queued",
            serde_json::json!({
                "session_id": session.id.to_string(),
                "workspace_id": session.workspace_id.to_string(),
            }),
        )
        .await;

    Ok(ResponseJson(ApiResponse::success(QueueStatus::Queued {
        message: queued,
    })))
}

/// Cancel a queued follow-up message
async fn cancel_queued_message(
    Extension(session): Extension<Session>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<QueueStatus>>, ApiError> {
    if deployment
        .queued_message_service()
        .has_mcp_restart(session.id)
    {
        supersede_mcp_session_restart(session.id, &deployment).await;
    }
    deployment
        .queued_message_service()
        .cancel_queued(session.id);

    deployment
        .track_if_analytics_allowed(
            "follow_up_queue_cancelled",
            serde_json::json!({
                "session_id": session.id.to_string(),
                "workspace_id": session.workspace_id.to_string(),
            }),
        )
        .await;

    Ok(ResponseJson(ApiResponse::success(QueueStatus::Empty)))
}

/// Get the current queue status for a session's workspace
async fn get_queue_status(
    Extension(session): Extension<Session>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<QueueStatus>>, ApiError> {
    let status = deployment.queued_message_service().get_status(session.id);

    Ok(ResponseJson(ApiResponse::success(status)))
}

pub(super) fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    Router::new()
        .route(
            "/",
            get(get_queue_status)
                .post(queue_message)
                .delete(cancel_queued_message),
        )
        .route("/mcp-restart", axum::routing::post(queue_mcp_restart))
        .route("/restart", axum::routing::post(restart_mcp_session_route))
        .layer(from_fn_with_state(
            deployment.clone(),
            load_session_middleware,
        ))
}
