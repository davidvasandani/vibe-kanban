use std::collections::HashMap;

use api_types::{
    CreateOrganizationEnvVarRequest, CreateOrganizationEnvVarResponse,
    ListOrganizationEnvVarsResponse, UpdateOrganizationEnvVarRequest,
    UpdateOrganizationEnvVarResponse, normalize_secret_reference,
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

/// Store a 1Password reference in its bare form (1Password copies it wrapped in
/// quotes); every other value is stored byte-for-byte. Returns the value to
/// store and, when it is a reference, that reference for the response.
fn prepare_value(value: String) -> Result<(String, Option<String>), ErrorResponse> {
    let reference = normalize_secret_reference(&value);
    let value = reference.clone().unwrap_or(value);
    if value.len() > ENV_VAR_VALUE_MAX_LEN {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "Env var value is too long",
        ));
    }
    Ok((value, reference))
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

    let repo = OrganizationEnvVarRepository::new(&state.pool);
    let mut env_vars = repo.list(org_id).await.map_err(map_env_var_error)?;
    // Names are unique per organization. Only values that are 1Password
    // references are surfaced; literal values never leave the server.
    let references: HashMap<String, String> = repo
        .list_with_encrypted_values(org_id)
        .await
        .map_err(map_env_var_error)?
        .into_iter()
        .filter_map(|(name, encrypted)| match state.jwt.decrypt_string(&encrypted) {
            Ok(value) => normalize_secret_reference(&value).map(|reference| (name, reference)),
            Err(error) => {
                tracing::warn!(?error, %name, "failed to decrypt org env var; showing it masked");
                None
            }
        })
        .collect();
    for env_var in &mut env_vars {
        env_var.reference = references.get(&env_var.name).cloned();
    }

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
    let (value, reference) = prepare_value(payload.value)?;

    let encrypted = state.jwt.encrypt_string(&value).map_err(|_| {
        ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to encrypt value")
    })?;

    let mut env_var = OrganizationEnvVarRepository::new(&state.pool)
        .create(org_id, name, &encrypted)
        .await
        .map_err(map_env_var_error)?;
    env_var.reference = reference;

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

    let (value, reference) = prepare_value(payload.value)?;

    let encrypted = state.jwt.encrypt_string(&value).map_err(|_| {
        ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to encrypt value")
    })?;

    let mut env_var = OrganizationEnvVarRepository::new(&state.pool)
        .update_value(org_id, id, &encrypted)
        .await
        .map_err(map_env_var_error)?;
    env_var.reference = reference;

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

#[cfg(test)]
mod tests {
    use super::prepare_value;

    #[test]
    fn quoted_reference_is_stored_bare_and_reported() {
        let (value, reference) =
            prepare_value("\"op://Homelab/alderbridge nix PAT/credential\"".to_string()).unwrap();
        assert_eq!(value, "op://Homelab/alderbridge nix PAT/credential");
        assert_eq!(reference.as_deref(), Some(value.as_str()));
    }

    #[test]
    fn literal_is_stored_unchanged_and_never_reported() {
        for literal in ["\"quoted literal\"", "  padded secret\n", "OP://not/a/ref"] {
            let (value, reference) = prepare_value(literal.to_string()).unwrap();
            assert_eq!(value, literal);
            assert_eq!(reference, None);
        }
    }

    #[test]
    fn oversized_value_is_rejected() {
        assert!(prepare_value("x".repeat(super::ENV_VAR_VALUE_MAX_LEN + 1)).is_err());
    }
}
