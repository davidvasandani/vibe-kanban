//! Machine-scoped GitHub owner tokens (Settings → Repositories). Literal
//! values are write-only; see `services::github_owner_tokens`.

use axum::{
    Router,
    extract::{Path, State},
    response::Json as ResponseJson,
    routing::{get, put},
};
use deployment::Deployment;
use services::services::github_owner_tokens::{
    self, CreateGitHubOwnerTokenRequest, GitHubOwnerToken, UpdateGitHubOwnerTokenRequest,
};
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/github-owner-tokens", get(list).post(create))
        .route("/github-owner-tokens/{id}", put(update).delete(remove))
}

async fn list(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<GitHubOwnerToken>>>, ApiError> {
    let tokens = github_owner_tokens::list(&deployment.db().pool).await?;
    Ok(ResponseJson(ApiResponse::success(tokens)))
}

async fn create(
    State(deployment): State<DeploymentImpl>,
    ResponseJson(request): ResponseJson<CreateGitHubOwnerTokenRequest>,
) -> Result<ResponseJson<ApiResponse<GitHubOwnerToken>>, ApiError> {
    let token = github_owner_tokens::create(&deployment.db().pool, request).await?;
    Ok(ResponseJson(ApiResponse::success(token)))
}

async fn update(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
    ResponseJson(request): ResponseJson<UpdateGitHubOwnerTokenRequest>,
) -> Result<ResponseJson<ApiResponse<GitHubOwnerToken>>, ApiError> {
    let token = github_owner_tokens::update(&deployment.db().pool, id, request).await?;
    Ok(ResponseJson(ApiResponse::success(token)))
}

async fn remove(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    github_owner_tokens::delete(&deployment.db().pool, id).await?;
    Ok(ResponseJson(ApiResponse::success(())))
}
