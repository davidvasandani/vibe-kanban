//! App-managed CLI tool installer endpoints (see services::cli_tools).
//!
//! All outcomes — including download/verification failures — are returned
//! in-band via the ApiResponse envelope so the settings UI can surface them.

use axum::{
    Router,
    extract::Path,
    response::Json as ResponseJson,
    routing::{delete, get, post},
};
use services::services::cli_tools::{self, CliToolId, CliToolStatus};
use utils::response::ApiResponse;

use crate::DeploymentImpl;

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/cli-tools", get(list_cli_tools))
        .route("/cli-tools/{id}/install", post(install_cli_tool))
        .route("/cli-tools/{id}/update", post(install_cli_tool))
        .route("/cli-tools/{id}", delete(remove_cli_tool))
}

async fn list_cli_tools() -> ResponseJson<ApiResponse<Vec<CliToolStatus>>> {
    ResponseJson(ApiResponse::success(cli_tools::status_all().await))
}

async fn install_cli_tool(Path(id): Path<CliToolId>) -> ResponseJson<ApiResponse<CliToolStatus>> {
    match cli_tools::install(id).await {
        Ok(status) => ResponseJson(ApiResponse::success(status)),
        Err(e) => {
            tracing::error!("CLI tool install failed: {e}");
            ResponseJson(ApiResponse::error(&e.to_string()))
        }
    }
}

async fn remove_cli_tool(Path(id): Path<CliToolId>) -> ResponseJson<ApiResponse<CliToolStatus>> {
    match cli_tools::remove(id).await {
        Ok(status) => ResponseJson(ApiResponse::success(status)),
        Err(e) => {
            tracing::error!("CLI tool removal failed: {e}");
            ResponseJson(ApiResponse::error(&e.to_string()))
        }
    }
}
