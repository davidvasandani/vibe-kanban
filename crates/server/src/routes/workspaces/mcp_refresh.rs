use axum::{
    Extension, Router,
    extract::{Path, State},
    response::Json as ResponseJson,
    routing::get,
};
use db::models::workspace::Workspace;
use deployment::Deployment;
use executors::mcp_refresh::McpRefreshResult;
use services::services::container::ContainerService;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

pub fn router() -> Router<DeploymentImpl> {
    Router::new().route("/{session_id}/mcp/refresh", get(status).post(refresh))
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
