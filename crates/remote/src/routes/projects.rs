use api_types::{
    BulkUpdateProjectsRequest, BulkUpdateProjectsResponse, CreateProjectRequest, DeleteResponse,
    ListProjectsQuery, ListProjectsResponse, MutationResponse, Project, ResolvedEnvVar,
    ResolvedOrganizationEnvVarsResponse, UpdateProjectRequest,
};
use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use tracing::instrument;
use uuid::Uuid;

use super::{
    error::{ErrorResponse, db_error},
    organization_members::ensure_member_access,
};
use crate::{
    AppState,
    auth::RequestContext,
    db::{
        get_txid, organization_env_vars::OrganizationEnvVarRepository, projects::ProjectRepository,
        types::is_valid_hsl_color,
    },
    mutation_definition::MutationBuilder,
};

/// Mutation definition for Projects - provides both router and TypeScript metadata.
pub fn mutation() -> MutationBuilder<Project, CreateProjectRequest, UpdateProjectRequest> {
    MutationBuilder::new("projects")
        .list(list_projects)
        .get(get_project)
        .create(create_project)
        .update(update_project)
        .delete(delete_project)
}

pub fn router() -> axum::Router<AppState> {
    mutation()
        .router()
        .route("/projects/bulk", post(bulk_update_projects))
        .route("/projects/{project_id}/env-vars", get(get_project_env_vars))
}

/// Returns the owning organization's env vars, decrypted, for a project the
/// caller can access. Used by agent hosts to inject org-level env vars (e.g.
/// GITHUB_TOKEN) into workspaces started against this project. Gated on org
/// membership — the same access required to read the project itself; any member
/// who can start a workspace here would receive these values in-process anyway.
#[instrument(
    name = "projects.get_project_env_vars",
    skip(state, ctx),
    fields(project_id = %project_id, user_id = %ctx.user.id)
)]
async fn get_project_env_vars(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<ResolvedOrganizationEnvVarsResponse>, ErrorResponse> {
    let project = ProjectRepository::find_by_id(state.pool(), project_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, %project_id, "failed to load project");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load project")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "project not found"))?;

    ensure_member_access(state.pool(), project.organization_id, ctx.user.id).await?;

    let pairs = OrganizationEnvVarRepository::new(state.pool())
        .list_with_encrypted_values(project.organization_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, organization_id = %project.organization_id, "failed to load org env vars");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load env vars")
        })?;

    let mut env_vars = Vec::with_capacity(pairs.len());
    for (name, encrypted) in pairs {
        match state.jwt.decrypt_string(&encrypted) {
            Ok(value) => env_vars.push(ResolvedEnvVar { name, value }),
            Err(error) => {
                // Skip individual undecryptable entries rather than failing the
                // whole workspace start.
                tracing::warn!(?error, %name, "failed to decrypt org env var; skipping");
            }
        }
    }

    Ok(Json(ResolvedOrganizationEnvVarsResponse { env_vars }))
}

#[instrument(
    name = "projects.list_projects",
    skip(state, ctx),
    fields(organization_id = %query.organization_id, user_id = %ctx.user.id)
)]
async fn list_projects(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Query(query): Query<ListProjectsQuery>,
) -> Result<Json<ListProjectsResponse>, ErrorResponse> {
    ensure_member_access(state.pool(), query.organization_id, ctx.user.id).await?;

    let projects = ProjectRepository::list_by_organization(state.pool(), query.organization_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, organization_id = %query.organization_id, "failed to list projects");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to list projects")
        })?;

    Ok(Json(ListProjectsResponse { projects }))
}

#[instrument(
    name = "projects.get_project",
    skip(state, ctx),
    fields(project_id = %project_id, user_id = %ctx.user.id)
)]
async fn get_project(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Project>, ErrorResponse> {
    let project = ProjectRepository::find_by_id(state.pool(), project_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, %project_id, "failed to load project");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load project")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "project not found"))?;

    ensure_member_access(state.pool(), project.organization_id, ctx.user.id).await?;

    Ok(Json(project))
}

#[instrument(
    name = "projects.create_project",
    skip(state, ctx, payload),
    fields(organization_id = %payload.organization_id, user_id = %ctx.user.id)
)]
async fn create_project(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<CreateProjectRequest>,
) -> Result<Json<MutationResponse<Project>>, ErrorResponse> {
    ensure_member_access(state.pool(), payload.organization_id, ctx.user.id).await?;

    if !is_valid_hsl_color(&payload.color) {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "Invalid color format. Expected HSL format: 'H S% L%'",
        ));
    }

    let response = ProjectRepository::create_with_defaults(
        state.pool(),
        payload.id,
        payload.organization_id,
        payload.name,
        payload.color,
    )
    .await
    .map_err(|error| {
        tracing::error!(?error, "failed to create project");
        db_error(error, "failed to create project")
    })?;

    if let Some(analytics) = state.analytics() {
        analytics.track(
            ctx.user.id,
            "project_created",
            serde_json::json!({
                "project_id": response.data.id,
                "organization_id": response.data.organization_id,
            }),
        );
    }

    Ok(Json(response))
}

#[instrument(
    name = "projects.update_project",
    skip(state, ctx, payload),
    fields(project_id = %project_id, user_id = %ctx.user.id)
)]
async fn update_project(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(project_id): Path<Uuid>,
    Json(payload): Json<UpdateProjectRequest>,
) -> Result<Json<MutationResponse<Project>>, ErrorResponse> {
    let existing = ProjectRepository::find_by_id(state.pool(), project_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, %project_id, "failed to load project");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load project")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "project not found"))?;

    ensure_member_access(state.pool(), existing.organization_id, ctx.user.id).await?;

    if let Some(ref color) = payload.color
        && !is_valid_hsl_color(color)
    {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "Invalid color format. Expected HSL format: 'H S% L%'",
        ));
    }

    let response = ProjectRepository::update(
        state.pool(),
        project_id,
        payload.name,
        payload.color,
        payload.context,
        payload.sort_order,
    )
    .await
    .map_err(|error| {
        tracing::error!(?error, "failed to update project");
        ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
    })?;

    Ok(Json(response))
}

#[instrument(
    name = "projects.bulk_update",
    skip(state, ctx, payload),
    fields(user_id = %ctx.user.id, count = payload.updates.len())
)]
async fn bulk_update_projects(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<BulkUpdateProjectsRequest>,
) -> Result<Json<BulkUpdateProjectsResponse>, ErrorResponse> {
    if payload.updates.is_empty() {
        return Ok(Json(BulkUpdateProjectsResponse {
            data: vec![],
            txid: 0,
        }));
    }

    let first_project = ProjectRepository::find_by_id(state.pool(), payload.updates[0].id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to find first project");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to find project")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "project not found"))?;

    let organization_id = first_project.organization_id;
    ensure_member_access(state.pool(), organization_id, ctx.user.id).await?;

    let mut tx = crate::db::begin_tx(state.pool()).await.map_err(|error| {
        tracing::error!(?error, "failed to begin transaction");
        ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
    })?;

    let mut results = Vec::with_capacity(payload.updates.len());

    for item in payload.updates {
        let project = ProjectRepository::find_by_id(&mut *tx, item.id)
            .await
            .map_err(|error| {
                tracing::error!(?error, project_id = %item.id, "failed to find project");
                ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to find project")
            })?
            .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "project not found"))?;

        if project.organization_id != organization_id {
            return Err(ErrorResponse::new(
                StatusCode::BAD_REQUEST,
                "all projects must belong to the same organization",
            ));
        }

        if let Some(ref color) = item.changes.color
            && !is_valid_hsl_color(color)
        {
            return Err(ErrorResponse::new(
                StatusCode::BAD_REQUEST,
                "Invalid color format. Expected HSL format: 'H S% L%'",
            ));
        }

        let updated = ProjectRepository::update_partial(
            &mut *tx,
            item.id,
            item.changes.name,
            item.changes.color,
            item.changes.context,
            item.changes.sort_order,
        )
        .await
        .map_err(|error| {
            tracing::error!(?error, project_id = %item.id, "failed to update project");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to update project",
            )
        })?;

        results.push(updated);
    }

    let txid = get_txid(&mut *tx).await.map_err(|error| {
        tracing::error!(?error, "failed to get txid");
        ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
    })?;
    tx.commit().await.map_err(|error| {
        tracing::error!(?error, "failed to commit transaction");
        ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
    })?;

    Ok(Json(BulkUpdateProjectsResponse {
        data: results,
        txid,
    }))
}

#[instrument(
    name = "projects.delete_project",
    skip(state, ctx),
    fields(project_id = %project_id, user_id = %ctx.user.id)
)]
async fn delete_project(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<DeleteResponse>, ErrorResponse> {
    let project = ProjectRepository::find_by_id(state.pool(), project_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, %project_id, "failed to load project");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load project")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "project not found"))?;

    ensure_member_access(state.pool(), project.organization_id, ctx.user.id).await?;

    let response = ProjectRepository::delete(state.pool(), project_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to delete project");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?;

    Ok(Json(response))
}
