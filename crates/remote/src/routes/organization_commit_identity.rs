use api_types::{
    GetOrganizationCommitIdentityResponse, OrganizationCommitIdentity,
    UpsertOrganizationCommitIdentityRequest, normalize_commit_identity,
};
use axum::{
    Json, Router,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use uuid::Uuid;

use super::{
    error::{ErrorResponse, db_error},
    organization_members::{ensure_admin_access, ensure_member_access},
};
use crate::{
    AppState, auth::RequestContext,
    db::organization_commit_identities::OrganizationCommitIdentityRepository,
};

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/organizations/{org_id}/commit-identity",
        get(get_commit_identity)
            .put(upsert_commit_identity)
            .delete(delete_commit_identity),
    )
}

/// Readable by any member: the identity is not secret, and every member's
/// workspaces commit with it.
async fn get_commit_identity(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(org_id): Path<Uuid>,
) -> Result<Json<GetOrganizationCommitIdentityResponse>, ErrorResponse> {
    ensure_member_access(&state.pool, org_id, ctx.user.id).await?;
    let commit_identity =
        OrganizationCommitIdentityRepository::find_by_organization(&state.pool, org_id)
            .await
            .map_err(|error| db_error(error, "failed to load commit identity"))?;
    Ok(Json(GetOrganizationCommitIdentityResponse {
        commit_identity,
    }))
}

async fn upsert_commit_identity(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(org_id): Path<Uuid>,
    Json(payload): Json<UpsertOrganizationCommitIdentityRequest>,
) -> Result<Json<OrganizationCommitIdentity>, ErrorResponse> {
    ensure_admin_access(&state.pool, org_id, ctx.user.id).await?;
    let identity = normalize_commit_identity(&payload)
        .map_err(|message| ErrorResponse::new(StatusCode::BAD_REQUEST, message))?;
    let saved =
        OrganizationCommitIdentityRepository::upsert(&state.pool, org_id, &identity, ctx.user.id)
            .await
            .map_err(|error| db_error(error, "failed to save commit identity"))?;
    Ok(Json(saved))
}

/// Idempotent: 204 whether or not an identity existed. Workspaces stop
/// enforcing it the next time they start.
async fn delete_commit_identity(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(org_id): Path<Uuid>,
) -> Result<impl IntoResponse, ErrorResponse> {
    ensure_admin_access(&state.pool, org_id, ctx.user.id).await?;
    OrganizationCommitIdentityRepository::delete(&state.pool, org_id)
        .await
        .map_err(|error| db_error(error, "failed to delete commit identity"))?;
    Ok(StatusCode::NO_CONTENT)
}
