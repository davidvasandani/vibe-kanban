//! Issue comment delivery to linked workspace sessions. The UI and the MCP
//! both go through `crate::issue_comment_delivery`, so the two surfaces share
//! one delivery path and report the same persisted outcome.

use axum::{
    Json, Router,
    extract::{Path, State},
    response::Json as ResponseJson,
    routing::{get, post},
};
use db::models::issue_comment_delivery::IssueCommentDelivery;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{
    DeploymentImpl,
    error::ApiError,
    issue_comment_delivery::{
        CreateIssueCommentResult, CreateIssueCommentWithIntent, IssueAgentTargets,
        IssueCommentsWithDelivery, RequestIssueCommentDelivery, WorkspaceIssueComments, dispatcher,
        issue_url,
    },
};

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/issues/{issue_id}/agent-targets", get(get_agent_targets))
        .route("/issues/{issue_id}/comments", get(get_issue_comments))
        .route(
            "/issues/{issue_id}/comment-deliveries",
            get(get_comment_deliveries),
        )
        .route("/issue-comment-deliveries", post(request_delivery))
        .route("/issue-comments", post(create_issue_comment))
        .route(
            "/issue-comments/by-workspace/{workspace_id}",
            get(get_workspace_issue_comments),
        )
}

async fn get_agent_targets(
    State(deployment): State<DeploymentImpl>,
    Path(issue_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<IssueAgentTargets>>, ApiError> {
    let targets = dispatcher(&deployment).agent_targets(issue_id).await?;
    Ok(ResponseJson(ApiResponse::success(targets)))
}

async fn get_issue_comments(
    State(deployment): State<DeploymentImpl>,
    Path(issue_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<IssueCommentsWithDelivery>>, ApiError> {
    let comments = dispatcher(&deployment).issue_comments(issue_id).await?;
    Ok(ResponseJson(ApiResponse::success(comments)))
}

async fn get_comment_deliveries(
    State(deployment): State<DeploymentImpl>,
    Path(issue_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<Vec<IssueCommentDelivery>>>, ApiError> {
    let deliveries = dispatcher(&deployment)
        .deliveries_for_issue(issue_id)
        .await?;
    Ok(ResponseJson(ApiResponse::success(deliveries)))
}

/// Deliver an existing comment, retry a failed delivery, or choose its target.
async fn request_delivery(
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<RequestIssueCommentDelivery>,
) -> Result<ResponseJson<ApiResponse<IssueCommentDelivery>>, ApiError> {
    let delivery = dispatcher(&deployment).request(payload).await?;
    Ok(ResponseJson(ApiResponse::success(delivery)))
}

/// Save a comment and, for a reply, deliver it (the MCP's create path).
async fn create_issue_comment(
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<CreateIssueCommentWithIntent>,
) -> Result<ResponseJson<ApiResponse<CreateIssueCommentResult>>, ApiError> {
    let (comment, issue, delivery) = dispatcher(&deployment).create_comment(payload).await?;
    Ok(ResponseJson(ApiResponse::success(
        CreateIssueCommentResult {
            comment,
            issue_url: issue.map(|issue| issue_url(issue.project_id, issue.id)),
            delivery,
        },
    )))
}

async fn get_workspace_issue_comments(
    State(deployment): State<DeploymentImpl>,
    Path(workspace_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<WorkspaceIssueComments>>, ApiError> {
    let comments = dispatcher(&deployment)
        .workspace_issue_comments(workspace_id)
        .await?;
    Ok(ResponseJson(ApiResponse::success(comments)))
}
