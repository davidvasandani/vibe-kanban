use axum::{
    Extension, Json, Router, extract::State, middleware::from_fn_with_state,
    response::Json as ResponseJson, routing::get,
};
use db::models::{scratch::DraftFollowUpData, session::Session};
use deployment::Deployment;
use executors::profile::ExecutorConfig;
use serde::{Deserialize, Serialize};
use services::services::{container::ContainerService, queued_message::QueueStatus};
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
enum QueueMcpRestartResult {
    ConfirmationRequired,
    Queued,
    Started,
}

async fn queue_mcp_restart(
    Extension(session): Extension<Session>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<QueueMcpRestartRequest>,
) -> Result<ResponseJson<ApiResponse<QueueMcpRestartResult>>, ApiError> {
    let was_running =
        db::models::execution_process::ExecutionProcess::has_running_coding_agent_for_session(
            &deployment.db().pool,
            session.id,
        )
        .await?;
    if was_running && !payload.confirmed_running_restart {
        return Ok(ResponseJson(ApiResponse::success(
            QueueMcpRestartResult::ConfirmationRequired,
        )));
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
            return Err(error.into());
        }
    };
    let queued = if running {
        if !payload.confirmed_running_restart {
            deployment
                .queued_message_service()
                .cancel_mcp_restart(session.id, reservation);
            return Ok(ResponseJson(ApiResponse::success(
                QueueMcpRestartResult::ConfirmationRequired,
            )));
        }
        let queued_at = deployment
            .queued_message_service()
            .commit_mcp_restart(session.id, reservation);
        let still_running_result =
            db::models::execution_process::ExecutionProcess::has_running_coding_agent_for_session(
                &deployment.db().pool,
                session.id,
            )
            .await;
        if matches!(still_running_result, Ok(true)) {
            return Ok(ResponseJson(ApiResponse::success(
                QueueMcpRestartResult::Queued,
            )));
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
        deployment
            .queued_message_service()
            .take_mcp_restart(session.id, reservation)
    };

    let result = if let Some(queued) = queued {
        deployment
            .container()
            .reap_warm_processes_for_session(session.id)
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
        .layer(from_fn_with_state(
            deployment.clone(),
            load_session_middleware,
        ))
}
