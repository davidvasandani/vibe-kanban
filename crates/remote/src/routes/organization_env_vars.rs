use api_types::{
    CreateOrganizationEnvVarRequest, CreateOrganizationEnvVarResponse,
    ListOrganizationEnvVarsResponse, UpdateOrganizationEnvVarRequest,
    UpdateOrganizationEnvVarResponse,
};
use axum::{
    Json, Router,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use uuid::Uuid;

use super::error::ErrorResponse;
use crate::{
    AppState,
    auth::RequestContext,
    db::{
        identity_errors::IdentityError,
        organization_env_vars::{OrganizationEnvVarError, OrganizationEnvVarRepository},
        organizations::OrganizationRepository,
    },
};

const ENV_VAR_NAME_MAX_LEN: usize = 256;
const ENV_VAR_VALUE_MAX_LEN: usize = 32_768;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/organizations/{org_id}/env-vars",
            get(list_env_vars).post(create_env_var),
        )
        .route(
            "/organizations/{org_id}/env-vars/{id}",
            axum::routing::patch(update_env_var).delete(delete_env_var),
        )
}

fn validate_name(name: &str) -> Result<&str, ErrorResponse> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "Env var name must not be empty",
        ));
    }
    if trimmed.len() > ENV_VAR_NAME_MAX_LEN {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "Env var name is too long",
        ));
    }
    let valid = trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_');
    let starts_with_digit = trimmed.chars().next().is_some_and(|c| c.is_ascii_digit());
    if !valid || starts_with_digit {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "Env var name must match [A-Za-z_][A-Za-z0-9_]*",
        ));
    }
    Ok(trimmed)
}

fn validate_value(value: &str) -> Result<&str, ErrorResponse> {
    if value.len() > ENV_VAR_VALUE_MAX_LEN {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "Env var value is too long",
        ));
    }
    Ok(value)
}

async fn assert_admin(state: &AppState, org_id: Uuid, user_id: Uuid) -> Result<(), ErrorResponse> {
    OrganizationRepository::new(&state.pool)
        .assert_admin(org_id, user_id)
        .await
        .map_err(|e| match e {
            IdentityError::PermissionDenied => {
                ErrorResponse::new(StatusCode::FORBIDDEN, "Admin access required")
            }
            IdentityError::NotFound => {
                ErrorResponse::new(StatusCode::NOT_FOUND, "Organization not found")
            }
            _ => ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "Database error"),
        })
}

fn map_env_var_error(err: OrganizationEnvVarError) -> ErrorResponse {
    match err {
        OrganizationEnvVarError::NameConflict => ErrorResponse::new(
            StatusCode::CONFLICT,
            "An env var with this name already exists",
        ),
        OrganizationEnvVarError::NotFound => {
            ErrorResponse::new(StatusCode::NOT_FOUND, "Env var not found")
        }
        OrganizationEnvVarError::Database(_) => {
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "Database error")
        }
    }
}

async fn list_env_vars(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(org_id): Path<Uuid>,
) -> Result<impl IntoResponse, ErrorResponse> {
    assert_admin(&state, org_id, ctx.user.id).await?;

    let env_vars = OrganizationEnvVarRepository::new(&state.pool)
        .list(org_id)
        .await
        .map_err(map_env_var_error)?;

    Ok(Json(ListOrganizationEnvVarsResponse { env_vars }))
}

async fn create_env_var(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(org_id): Path<Uuid>,
    Json(payload): Json<CreateOrganizationEnvVarRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    assert_admin(&state, org_id, ctx.user.id).await?;

    let name = validate_name(&payload.name)?;
    let value = validate_value(&payload.value)?;

    let encrypted = state.jwt.encrypt_string(value).map_err(|_| {
        ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to encrypt value")
    })?;

    let env_var = OrganizationEnvVarRepository::new(&state.pool)
        .create(org_id, name, &encrypted)
        .await
        .map_err(map_env_var_error)?;

    Ok((
        StatusCode::CREATED,
        Json(CreateOrganizationEnvVarResponse { env_var }),
    ))
}

async fn update_env_var(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path((org_id, id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateOrganizationEnvVarRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    assert_admin(&state, org_id, ctx.user.id).await?;

    let value = validate_value(&payload.value)?;

    let encrypted = state.jwt.encrypt_string(value).map_err(|_| {
        ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to encrypt value")
    })?;

    let env_var = OrganizationEnvVarRepository::new(&state.pool)
        .update_value(org_id, id, &encrypted)
        .await
        .map_err(map_env_var_error)?;

    Ok(Json(UpdateOrganizationEnvVarResponse { env_var }))
}

async fn delete_env_var(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path((org_id, id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, ErrorResponse> {
    assert_admin(&state, org_id, ctx.user.id).await?;

    OrganizationEnvVarRepository::new(&state.pool)
        .delete(org_id, id)
        .await
        .map_err(map_env_var_error)?;

    Ok(StatusCode::NO_CONTENT)
}
