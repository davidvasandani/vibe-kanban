use std::{
    collections::HashMap,
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    response::Json as ResponseJson,
    routing::get,
};
use chrono::Utc;
use db::models::{execution_process::ExecutionProcess, session::Session, workspace::Workspace};
use deployment::Deployment;
use executors::{
    mcp_recovery::{McpRecoveryResult, McpRecoveryScope, McpRecoveryStatus, McpRestartDisposition},
    mcp_refresh::{McpRefreshError, McpRefreshErrorCategory, McpRefreshResult},
};
use serde::Deserialize;
use services::services::container::ContainerService;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

pub fn router() -> Router<DeploymentImpl> {
    Router::new().route("/{session_id}/mcp/refresh", get(status).post(refresh))
}

#[derive(Debug, Default, Deserialize)]
pub struct RestartWorkspaceRequest {
    pub resume_session_id: Option<Uuid>,
    #[serde(default)]
    pub confirmed_running_restart: bool,
}

static MCP_WORKSPACE_RESTARTS: LazyLock<Mutex<HashMap<Uuid, McpRecoveryResult>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static MCP_WORKSPACE_RESTART_FORCE: LazyLock<Mutex<HashMap<Uuid, Arc<AtomicBool>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT_WORKSPACE_RECOVERY_GENERATION: AtomicU64 = AtomicU64::new(1);

struct WorkspaceRestartGuard {
    workspace_id: Uuid,
    active: bool,
}

struct RestartStartGate {
    service: services::services::queued_message::QueuedMessageService,
    session_id: Option<Uuid>,
}

impl Drop for RestartStartGate {
    fn drop(&mut self) {
        if let Some(session_id) = self.session_id {
            self.service.cancel_workspace_mcp_restart(session_id);
        }
    }
}

impl Drop for WorkspaceRestartGuard {
    fn drop(&mut self) {
        if self.active {
            {
                let mut operations = MCP_WORKSPACE_RESTARTS
                    .lock()
                    .expect("MCP workspace restart lock poisoned");
                if let Some(operation) = operations.get_mut(&self.workspace_id)
                    && matches!(
                        operation.status,
                        McpRecoveryStatus::Accepted | McpRecoveryStatus::InProgress
                    )
                {
                    operation.status = McpRecoveryStatus::Failed;
                    operation.completed_at = Some(Utc::now());
                }
            }
            MCP_WORKSPACE_RESTART_FORCE
                .lock()
                .expect("MCP workspace restart force lock poisoned")
                .remove(&self.workspace_id);
        }
    }
}

pub async fn workspace_restart_status(
    Extension(workspace): Extension<Workspace>,
) -> Result<ResponseJson<ApiResponse<Option<McpRecoveryResult>>>, ApiError> {
    let result = MCP_WORKSPACE_RESTARTS
        .lock()
        .expect("MCP workspace restart lock poisoned")
        .get(&workspace.id)
        .cloned();
    Ok(ResponseJson(ApiResponse::success(result)))
}

pub async fn restart_workspace(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<RestartWorkspaceRequest>,
) -> Result<ResponseJson<ApiResponse<McpRecoveryResult>>, ApiError> {
    if let Some(mut current) = MCP_WORKSPACE_RESTARTS
        .lock()
        .expect("MCP workspace restart lock poisoned")
        .get(&workspace.id)
        .filter(|operation| {
            matches!(
                operation.status,
                McpRecoveryStatus::Accepted | McpRecoveryStatus::InProgress
            )
        })
        .cloned()
    {
        if payload.confirmed_running_restart
            && let Some(force) = MCP_WORKSPACE_RESTART_FORCE
                .lock()
                .expect("MCP workspace restart force lock poisoned")
                .get(&workspace.id)
        {
            force.store(true, Ordering::Release);
        }
        current.status = McpRecoveryStatus::InProgress;
        current.disposition = McpRestartDisposition::AlreadyInProgress;
        return Ok(ResponseJson(ApiResponse::success(current)));
    }
    let session = match payload.resume_session_id {
        Some(session_id) => {
            let session = Session::find_by_id(&deployment.db().pool, session_id)
                .await?
                .ok_or_else(|| ApiError::Conflict("Session not found".to_string()))?;
            if session.workspace_id != workspace.id {
                return Err(ApiError::Conflict(
                    "Session does not belong to workspace".to_string(),
                ));
            }
            Some(session)
        }
        None => {
            let mut resumable = None;
            for candidate in
                Session::find_by_workspace_id(&deployment.db().pool, workspace.id).await?
            {
                let executions = ExecutionProcess::find_by_session_id(
                    &deployment.db().pool,
                    candidate.id,
                    false,
                )
                .await?;
                if executions.iter().any(|process| {
                    process.run_reason
                        == db::models::execution_process::ExecutionProcessRunReason::CodingAgent
                }) {
                    let running = ExecutionProcess::has_running_coding_agent_for_session(
                        &deployment.db().pool,
                        candidate.id,
                    )
                    .await?;
                    if resumable.is_none() || running {
                        resumable = Some(candidate);
                    }
                    if running {
                        break;
                    }
                }
            }
            resumable
        }
    };
    let executor = session
        .as_ref()
        .and_then(|session| session.executor.clone())
        .unwrap_or_else(|| "unknown".to_string());
    if let Some(session) = session.as_ref() {
        let executions =
            ExecutionProcess::find_by_session_id(&deployment.db().pool, session.id, false).await?;
        if !executions.iter().any(|process| {
            process.run_reason
                == db::models::execution_process::ExecutionProcessRunReason::CodingAgent
        }) {
            return Err(ApiError::Conflict(
                "Session has no coding-agent execution to resume".to_string(),
            ));
        }
    }
    let servers = if let Some(session) = session.as_ref() {
        deployment
            .container()
            .mcp_refresh_status(workspace.id, session.id)
            .await?
            .map(|status| status.servers)
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let result = McpRecoveryResult {
        generation: NEXT_WORKSPACE_RECOVERY_GENERATION.fetch_add(1, Ordering::Relaxed),
        scope: McpRecoveryScope::Workspace,
        workspace_id: workspace.id,
        session_id: session.as_ref().map(|session| session.id),
        status: McpRecoveryStatus::Accepted,
        disposition: McpRestartDisposition::Queued,
        requested_at: Utc::now(),
        completed_at: None,
        executor,
        servers,
        error: None,
    };

    let force_running_restart = Arc::new(AtomicBool::new(payload.confirmed_running_restart));
    {
        let mut operations = MCP_WORKSPACE_RESTARTS
            .lock()
            .expect("MCP workspace restart lock poisoned");
        if let Some(mut current) = operations
            .get(&workspace.id)
            .filter(|operation| {
                matches!(
                    operation.status,
                    McpRecoveryStatus::Accepted | McpRecoveryStatus::InProgress
                )
            })
            .cloned()
        {
            if payload.confirmed_running_restart
                && let Some(force) = MCP_WORKSPACE_RESTART_FORCE
                    .lock()
                    .expect("MCP workspace restart force lock poisoned")
                    .get(&workspace.id)
            {
                force.store(true, Ordering::Release);
            }
            current.status = McpRecoveryStatus::InProgress;
            current.disposition = McpRestartDisposition::AlreadyInProgress;
            return Ok(ResponseJson(ApiResponse::success(current)));
        }
        operations.insert(workspace.id, result.clone());
        MCP_WORKSPACE_RESTART_FORCE
            .lock()
            .expect("MCP workspace restart force lock poisoned")
            .insert(workspace.id, force_running_restart.clone());
    }
    let active_guard = WorkspaceRestartGuard {
        workspace_id: workspace.id,
        active: true,
    };

    // The backend owns this task. In particular, killing the requesting agent's
    // process group cannot cancel the workspace restart after this handler has
    // accepted it.
    let deployment_for_restart = deployment.clone();
    let workspace_for_restart = workspace.clone();
    let operation_generation = result.generation;
    let task_guard = active_guard;
    tokio::spawn(async move {
        let _guard = task_guard;
        let mut restart_start_gate = RestartStartGate {
            service: deployment_for_restart.queued_message_service().clone(),
            session_id: session.as_ref().map(|session| session.id),
        };
        if let Some(session_id) = restart_start_gate.session_id {
            restart_start_gate
                .service
                .block_mcp_restart_start(session_id);
        }
        let mut session_restart_queued = false;
        if let Some(session) = session.as_ref()
            && ExecutionProcess::has_running_coding_agent_for_session(
                &deployment_for_restart.db().pool,
                session.id,
            )
            .await
            .unwrap_or(false)
        {
            crate::routes::sessions::queue::supersede_mcp_session_restart(
                session.id,
                &deployment_for_restart,
            )
            .await;
            session_restart_queued = crate::routes::sessions::queue::restart_mcp_session(
                session,
                &deployment_for_restart,
            )
            .await
            .is_ok();
        }
        // A supplied session can be the caller even when MCP runs in global
        // mode, where there is no server-side scoped-session identity. Always
        // let that turn finish delivering the tool result before teardown.
        let grace_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match ExecutionProcess::find_running_coding_agents_by_workspace(
                &deployment_for_restart.db().pool,
                workspace_for_restart.id,
            )
            .await
            {
                Ok(processes) if processes.is_empty() => break,
                Ok(_) => {
                    if force_running_restart.load(Ordering::Acquire)
                        && tokio::time::Instant::now() >= grace_deadline
                    {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
                Err(error) => {
                    tracing::error!(workspace_id = %workspace_for_restart.id, %error, "Could not observe running coding agents before workspace restart");
                    return;
                }
            }
        }
        let workspace_launch_guard =
            services::services::container::lock_workspace_execution_starts(
                workspace_for_restart.id,
            )
            .await;
        let processes = match ExecutionProcess::find_all_running_by_workspace(
            &deployment_for_restart.db().pool,
            workspace_for_restart.id,
        )
        .await
        {
            Ok(processes) => processes,
            Err(error) => {
                tracing::error!(workspace_id = %workspace_for_restart.id, %error, "Could not enumerate workspace processes for MCP restart");
                return;
            }
        };
        if !session_restart_queued && let Some(session) = session.as_ref() {
            crate::routes::sessions::queue::supersede_mcp_session_restart(
                session.id,
                &deployment_for_restart,
            )
            .await;
        }
        let mut failed_stop_ids = std::collections::HashSet::new();
        let mut process_failures = Vec::new();
        if let Ok(workspace_sessions) = Session::find_by_workspace_id(
            &deployment_for_restart.db().pool,
            workspace_for_restart.id,
        )
        .await
        {
            for workspace_session in workspace_sessions {
                deployment_for_restart
                    .container()
                    .reap_warm_processes_for_session(workspace_session.id)
                    .await;
            }
        }
        for process in &processes {
            if !matches!(
                process.status,
                db::models::execution_process::ExecutionProcessStatus::Running
                    | db::models::execution_process::ExecutionProcessStatus::Indeterminate
            ) {
                continue;
            }
            if let Err(error) = deployment_for_restart
                .container()
                .stop_execution(
                    process,
                    db::models::execution_process::ExecutionProcessStatus::Killed,
                )
                .await
            {
                failed_stop_ids.insert(process.id);
                process_failures.push(format!("{}: stop failed", process.id));
                tracing::error!(execution_id = %process.id, %error, "Could not reconcile indeterminate process during workspace MCP restart");
            }
        }
        for process in &processes {
            let stop_confirmed =
                ExecutionProcess::find_by_id(&deployment_for_restart.db().pool, process.id)
                    .await
                    .ok()
                    .flatten()
                    .is_some_and(|current| {
                        !matches!(
                    current.status,
                    db::models::execution_process::ExecutionProcessStatus::Running
                        | db::models::execution_process::ExecutionProcessStatus::Indeterminate
                )
                    });
            if !stop_confirmed && failed_stop_ids.insert(process.id) {
                process_failures.push(format!("{}: stop could not be confirmed", process.id));
                tracing::error!(execution_id = %process.id, "Workspace process stop was not proven during MCP restart");
            }
        }

        let mut replay_failed = !failed_stop_ids.is_empty();
        for process in processes
            .iter()
            .filter(|process| process.run_reason.is_persistent())
        {
            let safely_stopped = !failed_stop_ids.contains(&process.id)
                && ExecutionProcess::find_by_id(&deployment_for_restart.db().pool, process.id)
                    .await
                    .ok()
                    .flatten()
                    .is_some_and(|current| {
                        !matches!(
                    current.status,
                    db::models::execution_process::ExecutionProcessStatus::Running
                        | db::models::execution_process::ExecutionProcessStatus::Indeterminate
                )
                    });
            if !safely_stopped {
                replay_failed = true;
                tracing::error!(execution_id = %process.id, "Persistent workspace process was not proven stopped; refusing to start a duplicate");
                continue;
            }
            let Ok(action) = process.executor_action().cloned() else {
                replay_failed = true;
                process_failures.push(format!("{}: launch definition unavailable", process.id));
                tracing::warn!(execution_id = %process.id, "Persistent workspace process has no replayable action");
                continue;
            };
            let Ok(Some(owner_session)) =
                Session::find_by_id(&deployment_for_restart.db().pool, process.session_id).await
            else {
                replay_failed = true;
                process_failures.push(format!("{}: owner session unavailable", process.id));
                tracing::warn!(execution_id = %process.id, "Persistent workspace process has no owner session");
                continue;
            };
            if let Err(error) = deployment_for_restart
                .container()
                .start_execution_during_workspace_restart(
                    &workspace_for_restart,
                    &owner_session,
                    &action,
                    &process.run_reason,
                )
                .await
            {
                replay_failed = true;
                process_failures.push(format!("{}: restart failed", process.id));
                tracing::error!(execution_id = %process.id, %error, "Could not recreate persistent workspace process after MCP restart");
            }
        }

        let coding_agents_stopped = {
            let mut all_stopped = true;
            for process in processes.iter().filter(|process| {
                process.run_reason
                    == db::models::execution_process::ExecutionProcessRunReason::CodingAgent
            }) {
                let stopped = !failed_stop_ids.contains(&process.id)
                    && ExecutionProcess::find_by_id(&deployment_for_restart.db().pool, process.id)
                        .await
                        .ok()
                        .flatten()
                        .is_some_and(|current| {
                            !matches!(
                        current.status,
                        db::models::execution_process::ExecutionProcessStatus::Running
                            | db::models::execution_process::ExecutionProcessStatus::Indeterminate
                    )
                        });
                all_stopped &= stopped;
            }
            all_stopped
        };
        if !coding_agents_stopped {
            tracing::error!(workspace_id = %workspace_for_restart.id, "Coding-agent stop was not proven; refusing to start a duplicate continuation");
            return;
        }

        drop(workspace_launch_guard);
        if let Some(session_id) = restart_start_gate.session_id.take() {
            restart_start_gate
                .service
                .unblock_mcp_restart_start(session_id);
        }

        if let Some(session) = session {
            let restart_result = if session_restart_queued {
                Ok(())
            } else {
                crate::routes::sessions::queue::restart_mcp_session(
                    &session,
                    &deployment_for_restart,
                )
                .await
                .map(|_| ())
            };
            match restart_result {
                Ok(_) => {
                    for _ in 0..35 {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        let Ok(Some(status)) = deployment_for_restart
                            .container()
                            .mcp_refresh_status(workspace_for_restart.id, session.id)
                            .await
                        else {
                            continue;
                        };
                        if matches!(
                            status.status,
                            executors::mcp_refresh::McpRefreshStatus::PendingNextTurn
                                | executors::mcp_refresh::McpRefreshStatus::Busy
                        ) {
                            continue;
                        }
                        let mut operations = MCP_WORKSPACE_RESTARTS
                            .lock()
                            .expect("MCP workspace restart lock poisoned");
                        if let Some(operation) = operations.get_mut(&workspace_for_restart.id)
                            && operation.generation == operation_generation
                        {
                            operation.servers = status.servers;
                            operation.error = status.error;
                            if replay_failed && operation.error.is_none() {
                                operation.error = Some(McpRefreshError {
                                    category: McpRefreshErrorCategory::Internal,
                                    message: format!(
                                        "Workspace process recovery was incomplete: {}.",
                                        process_failures.join(", ")
                                    ),
                                    remediation: "Inspect the named workspace processes, correct their launch or stop failure, then retry the workspace restart.".to_string(),
                                    retryable: true,
                                });
                            }
                            operation.completed_at = Some(Utc::now());
                            operation.status = if matches!(
                                status.status,
                                executors::mcp_refresh::McpRefreshStatus::Refreshed
                            ) && !replay_failed
                            {
                                McpRecoveryStatus::Completed
                            } else if matches!(
                                status.status,
                                executors::mcp_refresh::McpRefreshStatus::Failed
                                    | executors::mcp_refresh::McpRefreshStatus::Unsupported
                            ) {
                                McpRecoveryStatus::Failed
                            } else {
                                McpRecoveryStatus::PartiallyCompleted
                            };
                        }
                        return;
                    }
                    tracing::error!(session_id = %session.id, "Workspace MCP restart did not publish terminal inventory before its deadline");
                }
                Err(error) => {
                    tracing::error!(session_id = %session.id, %error, "Could not resume session after workspace MCP restart");
                }
            }
        } else {
            let mut operations = MCP_WORKSPACE_RESTARTS
                .lock()
                .expect("MCP workspace restart lock poisoned");
            if let Some(operation) = operations.get_mut(&workspace_for_restart.id)
                && operation.generation == operation_generation
            {
                operation.status = if replay_failed {
                    McpRecoveryStatus::PartiallyCompleted
                } else {
                    McpRecoveryStatus::Completed
                };
                if replay_failed {
                    operation.error = Some(McpRefreshError {
                        category: McpRefreshErrorCategory::Internal,
                        message: format!(
                            "Workspace process recovery was incomplete: {}.",
                            process_failures.join(", ")
                        ),
                        remediation: "Inspect the named workspace processes, correct their launch or stop failure, then retry the workspace restart.".to_string(),
                        retryable: true,
                    });
                }
                operation.completed_at = Some(Utc::now());
            }
        }
    });

    Ok(ResponseJson(ApiResponse::success(result)))
}

pub async fn refresh(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Path((_workspace_id, session_id)): Path<(Uuid, Uuid)>,
) -> Result<ResponseJson<ApiResponse<McpRefreshResult>>, ApiError> {
    let result = deployment
        .container()
        .refresh_mcp_tools(workspace.id, session_id)
        .await
        .map_err(ApiError::Container)?;
    Ok(ResponseJson(ApiResponse::success(result)))
}

pub async fn status(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Path((_workspace_id, session_id)): Path<(Uuid, Uuid)>,
) -> Result<ResponseJson<ApiResponse<Option<McpRefreshResult>>>, ApiError> {
    let result = deployment
        .container()
        .mcp_refresh_status(workspace.id, session_id)
        .await
        .map_err(ApiError::Container)?;
    Ok(ResponseJson(ApiResponse::success(result)))
}
