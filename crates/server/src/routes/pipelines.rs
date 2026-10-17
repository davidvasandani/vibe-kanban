//! HTTP routes for the file-based task pipelines
//! (`~/.vibe-kanban/pipelines/*.toml`). These feed the task-create pipeline
//! picker and the Settings → Pipeline editor. Parsing/validation lives in
//! `services::services::pipelines`; these handlers just map results to the
//! standard `ApiResponse` envelope (matching `config.rs`).

use std::time::Duration;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    response::Json as ResponseJson,
    routing::{get, post},
};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use services::services::pipelines::{
    self as pl, NewIssuePipelineDefaultsResponse, Pipeline, PipelineError, PipelineFileStatus,
    PipelineValidation,
};
use ts_rs::TS;
use utils::{path::pipelines_dir, response::ApiResponse};
use uuid::Uuid;

use crate::DeploymentImpl;

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/pipelines", get(list_pipelines))
        .route("/pipelines/status", get(list_pipeline_status))
        .route("/pipelines/new-issue-defaults", get(new_issue_defaults))
        .route("/pipelines/validate", post(validate_pipeline))
        .route("/pipelines/reset-defaults", post(reset_defaults))
        .route(
            "/pipelines/{id}/raw",
            get(get_pipeline_raw).put(put_pipeline_raw),
        )
        .route("/pipelines/{id}/reset", post(reset_pipeline))
        .route("/pipelines/{id}", axum::routing::delete(delete_pipeline))
}

/// Raw-TOML body for `PUT /pipelines/{id}/raw`.
#[derive(Debug, Serialize, Deserialize, TS)]
pub struct PipelineRawBody {
    pub content: String,
}

/// Body for `POST /pipelines/validate`. `id` defaults to a placeholder slug
/// since the draft may not have been saved under a real id yet.
#[derive(Debug, Serialize, Deserialize, TS)]
pub struct PipelineValidateBody {
    #[serde(default)]
    pub id: Option<String>,
    pub content: String,
}

/// Query for `GET /pipelines/new-issue-defaults`.
#[derive(Debug, Deserialize)]
pub struct NewIssueDefaultsQuery {
    #[serde(default)]
    pub project_id: Option<Uuid>,
}

/// Bound on the project → organization lookup, so resolving defaults never
/// stalls issue creation on a slow remote.
const ORG_LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);

/// Human-readable message for a pipeline error (surfaced to Settings Save, etc.).
fn err_message(e: &PipelineError) -> String {
    e.to_string()
}

async fn list_pipelines() -> ResponseJson<ApiResponse<Vec<Pipeline>>> {
    ResponseJson(ApiResponse::success(pl::load_pipelines(&pipelines_dir())))
}

/// The pipelines a new issue in `project_id` starts with (New Issue UI and
/// MCP `create_issue` without `pipeline_ids`). Never fails: an unknown
/// organization falls through to the machine default, and ids naming
/// pipelines that no longer load are dropped.
async fn new_issue_defaults(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<NewIssueDefaultsQuery>,
) -> ResponseJson<ApiResponse<NewIssuePipelineDefaultsResponse>> {
    let defaults = deployment.config().read().await.new_issue_pipelines.clone();

    // The organization only matters when the project has no override of
    // its own, so skip the remote round-trip otherwise.
    let org_id = match query.project_id {
        Some(project_id) if !defaults.projects.contains_key(&project_id) => {
            lookup_organization_id(&deployment, project_id).await
        }
        _ => None,
    };

    let (ids, source) = pl::resolve_new_issue_pipelines(&defaults, query.project_id, org_id);
    let pipeline_ids = pl::retain_available(&ids, &pl::load_pipelines(&pipelines_dir()));
    ResponseJson(ApiResponse::success(NewIssuePipelineDefaultsResponse {
        pipeline_ids,
        source,
    }))
}

async fn lookup_organization_id(deployment: &DeploymentImpl, project_id: Uuid) -> Option<Uuid> {
    let client = match deployment.remote_client() {
        Ok(client) => client,
        Err(_) => return None,
    };
    match tokio::time::timeout(ORG_LOOKUP_TIMEOUT, client.get_remote_project(project_id)).await {
        Ok(Ok(project)) => Some(project.organization_id),
        Ok(Err(e)) => {
            tracing::warn!(
                "Failed to look up organization for project {project_id}; \
                 skipping organization pipeline defaults: {e}"
            );
            None
        }
        Err(_) => {
            tracing::warn!(
                "Timed out looking up organization for project {project_id}; \
                 skipping organization pipeline defaults"
            );
            None
        }
    }
}

async fn list_pipeline_status() -> ResponseJson<ApiResponse<Vec<PipelineFileStatus>>> {
    ResponseJson(ApiResponse::success(pl::load_pipeline_statuses(
        &pipelines_dir(),
    )))
}

async fn validate_pipeline(
    Json(body): Json<PipelineValidateBody>,
) -> ResponseJson<ApiResponse<PipelineValidation>> {
    let id = body.id.as_deref().unwrap_or("pipeline");
    ResponseJson(ApiResponse::success(pl::validate(id, &body.content)))
}

async fn get_pipeline_raw(Path(id): Path<String>) -> ResponseJson<ApiResponse<String>> {
    match pl::read_raw(&pipelines_dir(), &id) {
        Ok(content) => ResponseJson(ApiResponse::success(content)),
        Err(e) => ResponseJson(ApiResponse::error(&err_message(&e))),
    }
}

async fn put_pipeline_raw(
    Path(id): Path<String>,
    Json(body): Json<PipelineRawBody>,
) -> ResponseJson<ApiResponse<Pipeline>> {
    match pl::write_raw(&pipelines_dir(), &id, &body.content) {
        Ok(pipeline) => ResponseJson(ApiResponse::success(pipeline)),
        Err(e) => ResponseJson(ApiResponse::error(&err_message(&e))),
    }
}

async fn reset_pipeline(Path(id): Path<String>) -> ResponseJson<ApiResponse<Pipeline>> {
    match pl::reset_one(&pipelines_dir(), &id) {
        Ok(pipeline) => ResponseJson(ApiResponse::success(pipeline)),
        Err(e) => ResponseJson(ApiResponse::error(&err_message(&e))),
    }
}

async fn reset_defaults() -> ResponseJson<ApiResponse<Vec<Pipeline>>> {
    match pl::reset_all(&pipelines_dir()) {
        Ok(pipelines) => ResponseJson(ApiResponse::success(pipelines)),
        Err(e) => ResponseJson(ApiResponse::error(&err_message(&e))),
    }
}

async fn delete_pipeline(Path(id): Path<String>) -> ResponseJson<ApiResponse<()>> {
    match pl::delete_pipeline(&pipelines_dir(), &id) {
        Ok(()) => ResponseJson(ApiResponse::success(())),
        Err(e) => ResponseJson(ApiResponse::error(&err_message(&e))),
    }
}
