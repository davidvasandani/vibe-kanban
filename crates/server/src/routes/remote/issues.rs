use api_types::{
    CreateIssueRequest, Issue, ListIssuesQuery, ListIssuesResponse, MutationResponse,
    ResolveLowDiskIssueRequest, ResolveLowDiskIssueResponse, SearchIssuesRequest,
    UpdateIssueRequest,
};
use axum::{
    Router,
    extract::{Json, Path, Query, State},
    response::Json as ResponseJson,
    routing::{get, post},
};
use deployment::Deployment;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/issues", get(list_issues).post(create_issue))
        .route("/issues/search", post(search_issues))
        .route("/issues/low-disk", post(resolve_low_disk_issue))
        .route(
            "/issues/{issue_id}",
            get(get_issue).patch(update_issue).delete(delete_issue),
        )
}

async fn resolve_low_disk_issue(
    State(deployment): State<DeploymentImpl>,
    Json(mut request): Json<ResolveLowDiskIssueRequest>,
) -> Result<ResponseJson<ApiResponse<ResolveLowDiskIssueResponse>>, ApiError> {
    let snapshot = deployment
        .cluster_metrics()
        .snapshot()
        .await
        .map_err(|error| match error {
            services::services::cluster::ClusterMetricsError::Database(error) => {
                ApiError::Database(error)
            }
        })?;
    let node = snapshot
        .nodes
        .get(&request.node_id)
        .ok_or_else(|| ApiError::BadRequest("metrics node not found".to_string()))?;
    if !matches!(
        node.availability,
        node_metrics::NodeMetricsAvailability::Available
            | node_metrics::NodeMetricsAvailability::Stale { .. }
    ) {
        return Err(ApiError::BadRequest(
            "node has no current or retained stale disk reading".to_string(),
        ));
    }
    let sample = node
        .latest
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("node has no current disk reading".to_string()))?;
    let thresholds = snapshot.disk_alert_thresholds;
    request.hostname = node.hostname.clone();
    request.observed_at = sample.captured_at;
    request.filesystems = sample
        .filesystems
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(|filesystem| {
            let (total, used, available) = (
                filesystem.total_bytes?,
                filesystem.used_bytes?,
                filesystem.available_bytes?,
            );
            if total == 0 {
                return None;
            }
            let free_percent = available as f64 / total as f64 * 100.0;
            let affected = free_percent < thresholds.warning_free_percent as f64
                || available < thresholds.warning_free_bytes;
            affected.then(|| api_types::LowDiskFilesystemObservation {
                device: filesystem.device.clone(),
                fs_type: filesystem.fs_type.clone(),
                mount_point: filesystem.mount_point.clone(),
                total_bytes: total,
                used_bytes: used,
                available_bytes: available,
            })
        })
        .collect();
    if request.filesystems.is_empty() {
        return Err(ApiError::BadRequest(
            "node no longer crosses the low-disk threshold".to_string(),
        ));
    }
    let client = deployment.remote_client()?;
    let response = client.resolve_low_disk_issue(&request).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn list_issues(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListIssuesQuery>,
) -> Result<ResponseJson<ApiResponse<ListIssuesResponse>>, ApiError> {
    let client = deployment.remote_client()?;
    let response = client.list_issues(query.project_id).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn search_issues(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<SearchIssuesRequest>,
) -> Result<ResponseJson<ApiResponse<ListIssuesResponse>>, ApiError> {
    let client = deployment.remote_client()?;
    let response = client.search_issues(&request).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn get_issue(
    State(deployment): State<DeploymentImpl>,
    Path(issue_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<Issue>>, ApiError> {
    let client = deployment.remote_client()?;
    let response = client.get_issue(issue_id).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn create_issue(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<CreateIssueRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<Issue>>>, ApiError> {
    let client = deployment.remote_client()?;
    let response = client.create_issue(&request).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn update_issue(
    State(deployment): State<DeploymentImpl>,
    Path(issue_id): Path<Uuid>,
    Json(request): Json<UpdateIssueRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<Issue>>>, ApiError> {
    let client = deployment.remote_client()?;
    let response = client.update_issue(issue_id, &request).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn delete_issue(
    State(deployment): State<DeploymentImpl>,
    Path(issue_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let client = deployment.remote_client()?;
    client.delete_issue(issue_id).await?;
    Ok(ResponseJson(ApiResponse::success(())))
}
