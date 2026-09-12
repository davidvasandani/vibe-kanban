use std::{
    collections::HashSet,
    sync::{LazyLock, Mutex},
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
    mcp_refresh::McpRefreshResult,
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
}

static ACTIVE_MCP_WORKSPACE_RESTARTS: LazyLock<Mutex<HashSet<Uuid>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

struct WorkspaceRestartGuard {
    workspace_id: Uuid,
    active: bool,
}

impl Drop for WorkspaceRestartGuard {
    fn drop(&mut self) {
        if self.active {
            ACTIVE_MCP_WORKSPACE_RESTARTS
                .lock()
                .expect("MCP workspace restart lock poisoned")
                .remove(&self.workspace_id);
        }
    }
}

pub async fn restart_workspace(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<RestartWorkspaceRequest>,
) -> Result<ResponseJson<ApiResponse<McpRecoveryResult>>, ApiError> {
    let already_active = {
        let mut active = ACTIVE_MCP_WORKSPACE_RESTARTS
            .lock()
            .expect("MCP workspace restart lock poisoned");
        !active.insert(workspace.id)
    };
    let active_guard = WorkspaceRestartGuard {
        workspace_id: workspace.id,
        active: !already_active,
    };
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
        None => Session::find_latest_by_workspace_id(&deployment.db().pool, workspace.id).await?,
    };
    let executor = session
        .as_ref()
        .and_then(|session| session.executor.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let servers = if let Some(session) = session.as_ref() {
        deployment
            .container()
            .prepare_mcp_restart(workspace.id, session.id)
            .await?
            .servers
    } else {
        Vec::new()
    };
    let result = McpRecoveryResult {
        generation: Utc::now().timestamp_millis().unsigned_abs(),
        scope: McpRecoveryScope::Workspace,
        workspace_id: workspace.id,
        session_id: session.as_ref().map(|session| session.id),
        status: if already_active {
            McpRecoveryStatus::InProgress
        } else {
            McpRecoveryStatus::Accepted
        },
        disposition: if already_active {
            McpRestartDisposition::AlreadyInProgress
        } else {
            McpRestartDisposition::Queued
        },
        requested_at: Utc::now(),
        completed_at: None,
        executor,
        servers,
        error: None,
    };

    if already_active {
        return Ok(ResponseJson(ApiResponse::success(result)));
    }

    // The backend owns this task. In particular, killing the requesting agent's
    // process group cannot cancel the workspace restart after this handler has
    // accepted it.
    let deployment_for_restart = deployment.clone();
    let workspace_for_restart = workspace.clone();
    let task_guard = active_guard;
    tokio::spawn(async move {
        let _guard = task_guard;
        // Give the HTTP/MCP transport a scheduling opportunity to flush the
        // accepted response before this operation stops the calling process.
        tokio::task::yield_now().await;
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
        deployment_for_restart
            .container()
            .try_stop(&workspace_for_restart, true)
            .await;

        for process in processes
            .iter()
            .filter(|process| process.run_reason.is_persistent())
        {
            let safely_stopped =
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
            if !safely_stopped {
                tracing::error!(execution_id = %process.id, "Persistent workspace process was not proven stopped; refusing to start a duplicate");
                continue;
            }
            let Ok(action) = process.executor_action().cloned() else {
                tracing::warn!(execution_id = %process.id, "Persistent workspace process has no replayable action");
                continue;
            };
            let Ok(Some(owner_session)) =
                Session::find_by_id(&deployment_for_restart.db().pool, process.session_id).await
            else {
                tracing::warn!(execution_id = %process.id, "Persistent workspace process has no owner session");
                continue;
            };
            if let Err(error) = deployment_for_restart
                .container()
                .start_execution(
                    &workspace_for_restart,
                    &owner_session,
                    &action,
                    &process.run_reason,
                )
                .await
            {
                tracing::error!(execution_id = %process.id, %error, "Could not recreate persistent workspace process after MCP restart");
            }
        }

        let coding_agents_stopped = {
            let mut all_stopped = true;
            for process in processes.iter().filter(|process| {
                process.run_reason
                    == db::models::execution_process::ExecutionProcessRunReason::CodingAgent
            }) {
                let stopped =
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
                all_stopped &= stopped;
            }
            all_stopped
        };
        if !coding_agents_stopped {
            tracing::error!(workspace_id = %workspace_for_restart.id, "Coding-agent stop was not proven; refusing to start a duplicate continuation");
            return;
        }

        if let Some(session) = session
            && let Err(error) = crate::routes::sessions::queue::restart_mcp_session(
                &session,
                &deployment_for_restart,
            )
            .await
        {
            tracing::error!(session_id = %session.id, %error, "Could not resume session after workspace MCP restart");
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
