//! Per-project Jira sync configuration endpoints.
//!
//! The credential is write-only: accepted in PUT/test payloads, stored
//! encrypted, and never included in any response (`has_credential` stands in
//! for it).

use axum::{
    Json, Router,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use uuid::Uuid;

use super::{error::ErrorResponse, organization_members::ensure_project_access};
use crate::{
    AppState,
    auth::RequestContext,
    db::jira_sync::{JiraSyncRepository, UpsertJiraSyncConfigArgs},
    jira::{
        client::{JiraClient, JiraClientError},
        types::{
            JiraAuthMode, JiraSyncConfig, JiraSyncConfigResponse, JiraSyncNowResponse,
            JiraTestConnectionRequest, JiraTestConnectionResponse, UpsertJiraSyncConfigRequest,
        },
    },
};

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/{project_id}/jira-sync",
            get(get_config)
                .put(upsert_config)
                .delete(delete_config),
        )
        .route(
            "/projects/{project_id}/jira-sync/test",
            post(test_connection),
        )
        .route(
            "/projects/{project_id}/jira-sync/sync-now",
            post(sync_now),
        )
}

fn internal(message: &str) -> ErrorResponse {
    ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, message)
}

fn bad_request(message: impl Into<String>) -> ErrorResponse {
    ErrorResponse::new(StatusCode::BAD_REQUEST, message)
}

async fn build_response(
    state: &AppState,
    config: JiraSyncConfig,
) -> Result<JiraSyncConfigResponse, ErrorResponse> {
    let link_counts = JiraSyncRepository::link_counts(&state.pool, config.project_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to load jira link counts");
            internal("failed to load link counts")
        })?;
    let auth_mode = JiraAuthMode::parse(&config.auth_mode)
        .ok_or_else(|| internal("stored auth mode is invalid"))?;
    Ok(JiraSyncConfigResponse {
        project_id: config.project_id,
        jira_base_url: config.jira_base_url,
        auth_mode,
        jira_email: config.jira_email,
        has_credential: !config.encrypted_credential.is_empty(),
        jql: config.jql,
        enabled: config.enabled,
        sync_interval_seconds: config.sync_interval_seconds,
        status_mapping: serde_json::from_value(config.status_mapping).unwrap_or_default(),
        sync_requested_at: config.sync_requested_at,
        last_sync_started_at: config.last_sync_started_at,
        last_sync_completed_at: config.last_sync_completed_at,
        last_sync_error: config.last_sync_error,
        link_counts,
    })
}

async fn get_config(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, project_id).await?;

    let config = JiraSyncRepository::find_config_by_project(&state.pool, project_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to load jira sync config");
            internal("failed to load Jira sync config")
        })?
        .ok_or_else(|| {
            ErrorResponse::new(StatusCode::NOT_FOUND, "no Jira sync config for project")
        })?;

    Ok(Json(build_response(&state, config).await?))
}

fn validate_upsert(payload: &UpsertJiraSyncConfigRequest) -> Result<(), ErrorResponse> {
    let url = payload.jira_base_url.trim();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(bad_request("Jira base URL must start with http(s)://"));
    }
    if payload.jql.trim().is_empty() {
        return Err(bad_request("JQL query must not be empty"));
    }
    if !(60..=3600).contains(&payload.sync_interval_seconds) {
        return Err(bad_request(
            "sync interval must be between 60 and 3600 seconds",
        ));
    }
    if payload.auth_mode == JiraAuthMode::CloudBasic
        && payload
            .jira_email
            .as_deref()
            .is_none_or(|e| e.trim().is_empty())
    {
        return Err(bad_request("email is required for Jira Cloud basic auth"));
    }
    Ok(())
}

async fn upsert_config(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(project_id): Path<Uuid>,
    Json(payload): Json<UpsertJiraSyncConfigRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, project_id).await?;
    validate_upsert(&payload)?;

    let existing = JiraSyncRepository::find_config_by_project(&state.pool, project_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to load jira sync config");
            internal("failed to load Jira sync config")
        })?;

    let credential = payload
        .credential
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty());
    if existing.is_none() && credential.is_none() {
        return Err(bad_request("a credential is required to create the sync"));
    }

    let encrypted_credential = credential
        .map(|c| state.jwt.encrypt_string(c))
        .transpose()
        .map_err(|_| internal("failed to encrypt credential"))?;

    let status_mapping = serde_json::to_value(&payload.status_mapping)
        .map_err(|_| bad_request("invalid status mapping"))?;

    let config = JiraSyncRepository::upsert_config(
        &state.pool,
        UpsertJiraSyncConfigArgs {
            project_id,
            jira_base_url: payload.jira_base_url.trim().trim_end_matches('/').to_string(),
            auth_mode: payload.auth_mode.as_str().to_string(),
            jira_email: payload.jira_email.clone(),
            encrypted_credential,
            jql: payload.jql.trim().to_string(),
            enabled: payload.enabled,
            sync_interval_seconds: payload.sync_interval_seconds,
            status_mapping,
            created_by_user_id: ctx.user.id,
        },
    )
    .await
    .map_err(|error| {
        tracing::error!(?error, "failed to save jira sync config");
        internal("failed to save Jira sync config")
    })?;

    Ok(Json(build_response(&state, config).await?))
}

async fn delete_config(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, project_id).await?;

    JiraSyncRepository::delete_config(&state.pool, project_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to delete jira sync config");
            internal("failed to delete Jira sync config")
        })?;

    // Idempotent: 204 whether or not a config existed. VK issues survive.
    Ok(StatusCode::NO_CONTENT)
}

async fn test_connection(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(project_id): Path<Uuid>,
    Json(payload): Json<JiraTestConnectionRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, project_id).await?;

    // Supplied credential wins; otherwise fall back to the stored one so the
    // user can re-test without re-typing the secret.
    let credential = match payload
        .credential
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
    {
        Some(supplied) => supplied.to_string(),
        None => {
            let stored = JiraSyncRepository::find_config_by_project(&state.pool, project_id)
                .await
                .map_err(|error| {
                    tracing::error!(?error, "failed to load jira sync config");
                    internal("failed to load Jira sync config")
                })?
                .filter(|c| !c.encrypted_credential.is_empty())
                .ok_or_else(|| bad_request("no credential supplied or stored"))?;
            state
                .jwt
                .decrypt_string(&stored.encrypted_credential)
                .map_err(|_| internal("failed to decrypt stored credential"))?
        }
    };

    let response = run_test(
        &state,
        &payload,
        credential,
    )
    .await;
    Ok(Json(response))
}

async fn run_test(
    state: &AppState,
    payload: &JiraTestConnectionRequest,
    credential: String,
) -> JiraTestConnectionResponse {
    let fail = |error: String| JiraTestConnectionResponse {
        ok: false,
        match_count: None,
        jira_statuses: Vec::new(),
        error: Some(error),
    };

    let client = match JiraClient::new(
        state.http_client.clone(),
        &payload.jira_base_url,
        payload.auth_mode,
        payload.jira_email.clone(),
        credential,
    ) {
        Ok(client) => client,
        Err(error) => return fail(error.to_string()),
    };

    if let Err(error) = client.myself().await {
        return fail(error.to_string());
    }

    match client.search_all(&payload.jql).await {
        Ok((issues, total)) => {
            let mut statuses: Vec<String> = issues
                .iter()
                .map(|i| i.status_name.clone())
                .filter(|s| !s.is_empty())
                .collect();
            statuses.sort();
            statuses.dedup();
            let match_count = match total {
                Some(total) => Some(total),
                None => match payload.auth_mode {
                    JiraAuthMode::CloudBasic => client
                        .approximate_count(&payload.jql)
                        .await
                        .or(Some(issues.len() as i64)),
                    JiraAuthMode::ServerPat => Some(issues.len() as i64),
                },
            };
            JiraTestConnectionResponse {
                ok: true,
                match_count,
                jira_statuses: statuses,
                error: None,
            }
        }
        Err(error @ JiraClientError::Jql(_)) => fail(error.to_string()),
        Err(error) => fail(error.to_string()),
    }
}

async fn sync_now(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, project_id).await?;

    let config = JiraSyncRepository::find_config_by_project(&state.pool, project_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to load jira sync config");
            internal("failed to load Jira sync config")
        })?
        .ok_or_else(|| {
            ErrorResponse::new(StatusCode::NOT_FOUND, "no Jira sync config for project")
        })?;
    if !config.enabled {
        return Err(ErrorResponse::new(
            StatusCode::CONFLICT,
            "Jira sync is disabled for this project",
        ));
    }

    let requested_at = JiraSyncRepository::request_sync_now(&state.pool, project_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to request jira sync");
            internal("failed to request sync")
        })?
        .ok_or_else(|| internal("failed to request sync"))?;

    Ok((
        StatusCode::ACCEPTED,
        Json(JiraSyncNowResponse { requested_at }),
    ))
}
