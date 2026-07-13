//! Slack integration endpoints.
//!
//! Admin routes (protected, org admin): connect/read/disconnect/test the
//! org's Slack workspace config. Credentials are write-only — accepted in
//! PUT payloads, stored encrypted, and never included in any response
//! (`has_credentials` stands in for them), matching the Jira precedent.
//!
//! Inbound route (public): `POST /slack/interactivity` receives Slack
//! interaction payloads. Order of operations is normative (constitution
//! inbound-endpoint rule): peek at `team.id` only (side-effect-free) to
//! find the config, verify the request signature with that config's
//! signing secret, and only then act. A team with no config gets an empty
//! 200 — there is no secret to verify against and no token to answer
//! with, and replying via the unverified payload's `response_url` would
//! hand responses to forged requests.

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Extension, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::json;
use tracing::warn;
use uuid::Uuid;

use super::error::ErrorResponse;
use crate::{
    AppState,
    anthropic::{
        client::AnthropicClient,
        prompt::{MAX_THREAD_MESSAGES, MAX_THREAD_PAGES},
    },
    auth::RequestContext,
    db::{
        identity_errors::IdentityError,
        issues::IssueRepository,
        organizations::OrganizationRepository,
        project_statuses::ProjectStatusRepository,
        projects::ProjectRepository,
        slack_configs::{SlackConfigDbError, SlackConfigRepository, UpsertSlackConfigArgs},
    },
    slack::{
        client::SlackClient,
        modal::{self, ProjectOption},
        prefill,
        signature::verify_slack_signature,
        types::{
            CREATE_ISSUE_MODAL_CALLBACK_ID, InteractionPeek, MESSAGE_SHORTCUT_CALLBACK_ID,
            MessageActionPayload, ModalMetadata, SlackConfig, SlackConfigResponse,
            SlackTestConnectionResponse, UpsertSlackConfigRequest, ViewSubmissionPayload,
        },
    },
};

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/organizations/{org_id}/slack",
            get(get_config).put(upsert_config).delete(delete_config),
        )
        .route("/organizations/{org_id}/slack/test", post(test_connection))
}

pub(super) fn public_router() -> Router<AppState> {
    Router::new().route("/slack/interactivity", post(handle_interactivity))
}

fn internal(message: &str) -> ErrorResponse {
    ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, message)
}

fn bad_request(message: impl Into<String>) -> ErrorResponse {
    ErrorResponse::new(StatusCode::BAD_REQUEST, message)
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

fn interactivity_url(state: &AppState) -> String {
    format!(
        "{}/v1/slack/interactivity",
        state.server_public_base_url.trim_end_matches('/')
    )
}

fn build_response(state: &AppState, config: SlackConfig) -> SlackConfigResponse {
    let has_anthropic_api_key = config
        .encrypted_anthropic_api_key
        .as_deref()
        .is_some_and(|k| !k.is_empty());
    SlackConfigResponse {
        organization_id: config.organization_id,
        slack_team_id: config.slack_team_id,
        slack_team_name: config.slack_team_name,
        enabled: config.enabled,
        has_credentials: !config.encrypted_bot_token.is_empty()
            && !config.encrypted_signing_secret.is_empty(),
        interactivity_url: interactivity_url(state),
        ai_summarization_enabled: config.ai_summarization_enabled,
        has_anthropic_api_key,
        created_at: config.created_at,
        updated_at: config.updated_at,
    }
}

// ---------------------------------------------------------------------------
// Admin REST
// ---------------------------------------------------------------------------

async fn get_config(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(org_id): Path<Uuid>,
) -> Result<impl IntoResponse, ErrorResponse> {
    assert_admin(&state, org_id, ctx.user.id).await?;

    let config = SlackConfigRepository::find_by_organization(&state.pool, org_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to load slack config");
            internal("failed to load Slack config")
        })?
        .ok_or_else(|| {
            ErrorResponse::new(StatusCode::NOT_FOUND, "no Slack config for organization")
        })?;

    Ok(Json(build_response(&state, config)))
}

async fn upsert_config(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(org_id): Path<Uuid>,
    Json(payload): Json<UpsertSlackConfigRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    assert_admin(&state, org_id, ctx.user.id).await?;

    let bot_token = payload
        .bot_token
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let signing_secret = payload
        .signing_secret
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let existing = SlackConfigRepository::find_by_organization(&state.pool, org_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to load slack config");
            internal("failed to load Slack config")
        })?;
    if existing.is_none() && (bot_token.is_none() || signing_secret.is_none()) {
        return Err(bad_request(
            "bot token and signing secret are both required to connect Slack",
        ));
    }

    // A new/changed bot token is validated against Slack before anything is
    // stored; auth.test also gives us the workspace this token belongs to,
    // which becomes the routing key for inbound payloads.
    let team = match bot_token {
        Some(token) => {
            let client = SlackClient::new(state.http_client.clone(), token.to_string());
            let auth = client
                .auth_test()
                .await
                .map_err(|e| bad_request(format!("slack_auth_failed: {e}")))?;
            let team_id = auth
                .team_id
                .filter(|t| !t.is_empty())
                .ok_or_else(|| bad_request("Slack did not identify the workspace"))?;
            let team_name = auth.team.unwrap_or_else(|| team_id.clone());
            Some((team_id, team_name))
        }
        None => None,
    };

    let encrypted_bot_token = bot_token
        .map(|t| state.jwt.encrypt_string(t))
        .transpose()
        .map_err(|_| internal("failed to encrypt credential"))?;
    let encrypted_signing_secret = signing_secret
        .map(|s| state.jwt.encrypt_string(s))
        .transpose()
        .map_err(|_| internal("failed to encrypt credential"))?;
    let (slack_team_id, slack_team_name) = team.unzip();

    // Anthropic key: write-only, encrypted like the bot token. Empty/absent
    // keeps the stored value (COALESCE in the repo). Not validated at save —
    // a bad key degrades to the mechanical prefill at first use (FR-5).
    let anthropic_api_key = payload
        .anthropic_api_key
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty());
    let encrypted_anthropic_api_key = anthropic_api_key
        .map(|k| state.jwt.encrypt_string(k))
        .transpose()
        .map_err(|_| internal("failed to encrypt credential"))?;

    let config = SlackConfigRepository::upsert(
        &state.pool,
        UpsertSlackConfigArgs {
            organization_id: org_id,
            encrypted_bot_token,
            encrypted_signing_secret,
            slack_team_id,
            slack_team_name,
            enabled: payload.enabled,
            created_by_user_id: ctx.user.id,
            encrypted_anthropic_api_key,
            // Omitted ⇒ keep the stored value (the repo sets this column
            // directly, not via COALESCE).
            ai_summarization_enabled: payload.ai_summarization_enabled.unwrap_or_else(|| {
                existing
                    .as_ref()
                    .map(|c| c.ai_summarization_enabled)
                    .unwrap_or(false)
            }),
        },
    )
    .await
    .map_err(|error| match error {
        SlackConfigDbError::TeamAlreadyConnected => ErrorResponse::new(
            StatusCode::CONFLICT,
            "this Slack workspace is already connected to another organization",
        ),
        error => {
            tracing::error!(?error, "failed to save slack config");
            internal("failed to save Slack config")
        }
    })?;

    Ok(Json(build_response(&state, config)))
}

async fn delete_config(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(org_id): Path<Uuid>,
) -> Result<impl IntoResponse, ErrorResponse> {
    assert_admin(&state, org_id, ctx.user.id).await?;

    SlackConfigRepository::delete(&state.pool, org_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to delete slack config");
            internal("failed to delete Slack config")
        })?;

    // Idempotent: 204 whether or not a config existed. Issues created from
    // Slack survive disconnection (FR-12).
    Ok(StatusCode::NO_CONTENT)
}

async fn test_connection(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(org_id): Path<Uuid>,
) -> Result<impl IntoResponse, ErrorResponse> {
    assert_admin(&state, org_id, ctx.user.id).await?;

    let config = SlackConfigRepository::find_by_organization(&state.pool, org_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to load slack config");
            internal("failed to load Slack config")
        })?
        .ok_or_else(|| bad_request("no Slack config stored for this organization"))?;

    let bot_token = state
        .jwt
        .decrypt_string(&config.encrypted_bot_token)
        .map_err(|_| internal("failed to decrypt stored credential"))?;

    let client = SlackClient::new(state.http_client.clone(), bot_token);
    let response = match client.auth_test().await {
        Ok(auth) => SlackTestConnectionResponse {
            ok: true,
            team_name: auth.team.or(auth.team_id),
            error: None,
        },
        Err(error) => SlackTestConnectionResponse {
            ok: false,
            team_name: None,
            error: Some(error.to_string()),
        },
    };
    Ok(Json(response))
}

// ---------------------------------------------------------------------------
// Inbound interactivity
// ---------------------------------------------------------------------------

/// Extract the `payload` field from Slack's
/// `application/x-www-form-urlencoded` body.
fn extract_payload_field(body: &[u8]) -> Option<String> {
    url::form_urlencoded::parse(body)
        .find(|(key, _)| key == "payload")
        .map(|(_, value)| value.into_owned())
}

/// POST /v1/slack/interactivity
async fn handle_interactivity(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(payload_json) = extract_payload_field(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(peek) = serde_json::from_str::<InteractionPeek>(&payload_json) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(team_id) = peek.team.as_ref().map(|t| t.id.as_str()) else {
        return StatusCode::OK.into_response();
    };

    let config = match SlackConfigRepository::find_by_team(&state.pool, team_id).await {
        Ok(Some(config)) => config,
        Ok(None) => {
            // No config → no secret to verify against, no token to answer
            // with. Stay silent (spec FR-7 exception).
            return StatusCode::OK.into_response();
        }
        Err(error) => {
            tracing::error!(?error, "failed to load slack config for team");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let Ok(signing_secret) = state.jwt.decrypt_string(&config.encrypted_signing_secret) else {
        tracing::error!("failed to decrypt slack signing secret");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let signature = headers
        .get("X-Slack-Signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let timestamp = headers
        .get("X-Slack-Request-Timestamp")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !verify_slack_signature(
        signing_secret.as_bytes(),
        signature,
        timestamp,
        &body,
        chrono::Utc::now().timestamp(),
    ) {
        warn!("invalid slack request signature");
        return StatusCode::UNAUTHORIZED.into_response();
    }

    match peek.kind.as_str() {
        "message_action" => handle_message_action(&state, &config, &payload_json).await,
        "view_submission" => handle_view_submission(&state, &config, &payload_json).await,
        _ => StatusCode::OK.into_response(),
    }
}

/// The user ran "Create issue from message": ack immediately, then open the
/// modal from a spawned task. Slack's interaction deadline is 3 seconds and
/// the shared HTTP client's timeout is far longer, so the DB query and the
/// `views.open` call must not sit between receipt and the HTTP 200 — the
/// `trigger_id` the spawned task uses has the same 3-second lifetime, but a
/// slow open then fails alone instead of also voiding the ack.
async fn handle_message_action(
    state: &AppState,
    config: &SlackConfig,
    payload_json: &str,
) -> Response {
    let Ok(action) = serde_json::from_str::<MessageActionPayload>(payload_json) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if action.callback_id != MESSAGE_SHORTCUT_CALLBACK_ID {
        return StatusCode::OK.into_response();
    }

    let Ok(bot_token) = state.jwt.decrypt_string(&config.encrypted_bot_token) else {
        tracing::error!("failed to decrypt slack bot token");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let state = state.clone();
    let organization_id = config.organization_id;
    let enabled = config.enabled;
    // Whether the AI summary will run (FR-11), and the encrypted key it needs
    // (decrypted inside the spawned task, off the ack path).
    let ai_active = config.ai_summarization_active();
    let encrypted_anthropic_api_key = config.encrypted_anthropic_api_key.clone();
    tokio::spawn(async move {
        open_shortcut_modal(
            state,
            organization_id,
            enabled,
            ai_active,
            encrypted_anthropic_api_key,
            bot_token,
            action,
        )
        .await;
    });
    StatusCode::OK.into_response()
}

/// While the AI summary is generated, the initial modal carries this hint so
/// the user knows an update is coming (FR-8).
const SUMMARIZING_HINT: &str = "✨ Summarizing this thread — the title and description will \
update in a moment.";

/// The deferred half of `handle_message_action`: decide which modal to show
/// and open it, then (when AI is active) fetch the thread, summarize it, and
/// swap the summary into the open modal. Failures here are delivery/enrichment
/// problems (logged); the request was already acked, and any AI failure leaves
/// the mechanical prefill in place (FR-5).
#[allow(clippy::too_many_arguments)]
async fn open_shortcut_modal(
    state: AppState,
    organization_id: Uuid,
    enabled: bool,
    ai_active: bool,
    encrypted_anthropic_api_key: Option<String>,
    bot_token: String,
    action: MessageActionPayload,
) {
    let client = SlackClient::new(state.http_client.clone(), bot_token);

    // FR-7: a disabled connection still answers the human who clicked.
    if !enabled {
        let view = modal::build_info_modal(
            "The Vibe Kanban Slack integration is currently disabled. \
             An organization admin can re-enable it in Vibe Kanban settings.",
        );
        if let Err(error) = client.views_open(&action.trigger_id, view).await {
            warn!(%error, "failed to open disabled-info modal");
        }
        return;
    }

    let projects = match ProjectRepository::list_by_organization(&state.pool, organization_id).await
    {
        Ok(projects) => projects,
        Err(error) => {
            tracing::error!(?error, "failed to list projects for slack modal");
            return;
        }
    };
    if projects.is_empty() {
        let view = modal::build_info_modal(
            "There are no projects in the connected Vibe Kanban organization yet. \
             Create a project in Vibe Kanban first.",
        );
        if let Err(error) = client.views_open(&action.trigger_id, view).await {
            warn!(%error, "failed to open no-projects modal");
        }
        return;
    }

    let message_text = action.message.text.as_deref().unwrap_or("");
    let permalink = prefill::build_permalink(
        action.team.domain.as_deref(),
        &action.channel.id,
        action.message.ts.as_deref(),
    );
    let metadata = ModalMetadata {
        team_id: action.team.id.clone(),
        team_domain: action.team.domain.clone(),
        channel_id: action.channel.id.clone(),
        message_ts: action.message.ts.clone(),
        permalink: permalink.clone(),
        slack_user_id: action.user.id.clone(),
        slack_user_name: action.user.display_name(),
    };
    let Ok(private_metadata) = serde_json::to_string(&metadata) else {
        tracing::error!("failed to serialize slack modal metadata");
        return;
    };

    let options: Vec<ProjectOption> = projects
        .iter()
        .map(|p| ProjectOption {
            id: p.id,
            name: p.name.clone(),
        })
        .collect();

    // Build the mechanical prefill (spec FR-2/FR-3) — shown immediately,
    // independently of the AI path. When AI is active, carry the hint so the
    // user is told an update is coming before the single follow-up lands.
    let mechanical_title = prefill::title_from_message(message_text);
    let mechanical_description =
        prefill::description_from_message(message_text, permalink.as_deref());
    let hint = ai_active.then_some(SUMMARIZING_HINT);
    let created = modal::build_create_issue_modal(
        &options,
        &mechanical_title,
        &mechanical_description,
        &private_metadata,
        hint,
        false,
    );
    if created.truncated_projects > 0 {
        warn!(
            organization_id = %organization_id,
            truncated = created.truncated_projects,
            "slack project selector truncated at Slack's 100-option cap"
        );
    }

    let view_id = match client.views_open(&action.trigger_id, created.view).await {
        Ok(view_id) => view_id,
        Err(error) => {
            warn!(%error, "failed to open create-issue modal");
            return;
        }
    };

    // The mechanical modal is open and usable. Everything below is optional
    // enrichment; nothing here can block or undo the modal (FR-2/FR-5).
    if !ai_active {
        return;
    }
    let Some(view_id) = view_id else {
        // No view id to update — leave the (hinted) mechanical modal as-is.
        warn!("slack views.open returned no view id; skipping AI summary");
        return;
    };

    let summary = summarize_thread_for_modal(
        &state,
        &client,
        encrypted_anthropic_api_key.as_deref(),
        &action,
    )
    .await;

    match summary {
        Some(summary) => {
            let ai_title = prefill::title_from_message(&summary.title);
            let ai_description =
                prefill::description_from_message(&summary.description, permalink.as_deref());
            // Drop the hint and swap in the AI title/description. `ai_variant
            // = true` uses fresh input ids so Slack actually shows the new
            // values on views.update (input-state preservation gotcha).
            let updated = modal::build_create_issue_modal(
                &options,
                &ai_title,
                &ai_description,
                &private_metadata,
                None,
                true,
            );
            if let Err(error) = client.views_update(&view_id, updated.view).await {
                warn!(%error, "failed to apply AI summary to slack modal");
            }
        }
        None => {
            // Degrade (FR-5): re-render the mechanical modal without the hint
            // so no stale "Summarizing…" notice lingers.
            let reverted = modal::build_create_issue_modal(
                &options,
                &mechanical_title,
                &mechanical_description,
                &private_metadata,
                None,
                false,
            );
            if let Err(error) = client.views_update(&view_id, reverted.view).await {
                warn!(%error, "failed to clear AI summarizing hint after failure");
            }
        }
    }
}

/// Fetch the target thread and ask Anthropic for a title/description. Returns
/// `None` on any failure (thread fetch, key decrypt, provider error); the
/// caller degrades to the mechanical prefill (FR-5). Logs are error-class only
/// — never the thread transcript or the API key (FR-15).
async fn summarize_thread_for_modal(
    state: &AppState,
    client: &SlackClient,
    encrypted_anthropic_api_key: Option<&str>,
    action: &MessageActionPayload,
) -> Option<crate::anthropic::types::IssueSummary> {
    // A threaded reply summarizes its parent thread; a standalone message
    // summarizes itself (FR-3).
    let thread_ts = action
        .message
        .thread_ts
        .as_deref()
        .or(action.message.ts.as_deref())
        .filter(|t| !t.is_empty())?;

    let messages = match client
        .conversations_replies(
            &action.channel.id,
            thread_ts,
            MAX_THREAD_MESSAGES,
            MAX_THREAD_PAGES,
        )
        .await
    {
        Ok(messages) => messages,
        Err(error) => {
            warn!(%error, "slack thread fetch for AI summary failed; using mechanical prefill");
            return None;
        }
    };

    let api_key = match encrypted_anthropic_api_key.map(|k| state.jwt.decrypt_string(k)) {
        Some(Ok(key)) => key,
        Some(Err(_)) => {
            warn!("failed to decrypt anthropic api key; using mechanical prefill");
            return None;
        }
        None => return None,
    };

    let anthropic = AnthropicClient::new(state.http_client.clone(), api_key);
    match anthropic.summarize_thread(&messages).await {
        Ok(summary) => Some(summary),
        Err(error) => {
            // `error` is an error class / Anthropic message — never the key or
            // the thread text (FR-15).
            warn!(%error, "anthropic thread summary failed; using mechanical prefill");
            None
        }
    }
}

/// In-modal validation errors for a `view_submission` (Slack renders each
/// message under the block with the matching id).
fn submission_errors(block_id: &str, message: &str) -> Response {
    Json(json!({
        "response_action": "errors",
        "errors": { block_id: message },
    }))
    .into_response()
}

/// The user submitted the modal: create the issue (one insert, inside the
/// ack window), then confirm asynchronously.
async fn handle_view_submission(
    state: &AppState,
    config: &SlackConfig,
    payload_json: &str,
) -> Response {
    let Ok(submission) = serde_json::from_str::<ViewSubmissionPayload>(payload_json) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if submission.view.callback_id != CREATE_ISSUE_MODAL_CALLBACK_ID {
        return StatusCode::OK.into_response();
    }

    let state_values = &submission.view.state;

    // The submitted view uses either the mechanical input ids or the AI-variant
    // ids (a views.update to the AI summary re-renders with fresh ids — see
    // `modal.rs`). Read from whichever is present, and remember the title
    // block id so in-modal errors target a block that actually exists.
    let (title_block, title) = state_values
        .text_input(modal::TITLE_BLOCK_ID, modal::TITLE_ACTION_ID)
        .map(|t| (modal::TITLE_BLOCK_ID, t))
        .or_else(|| {
            state_values
                .text_input(modal::TITLE_BLOCK_ID_AI, modal::TITLE_ACTION_ID_AI)
                .map(|t| (modal::TITLE_BLOCK_ID_AI, t))
        })
        .unwrap_or((modal::TITLE_BLOCK_ID, String::new()));
    let title = title.trim().to_string();
    let description = state_values
        .text_input(modal::DESCRIPTION_BLOCK_ID, modal::DESCRIPTION_ACTION_ID)
        .or_else(|| {
            state_values.text_input(
                modal::DESCRIPTION_BLOCK_ID_AI,
                modal::DESCRIPTION_ACTION_ID_AI,
            )
        })
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty());

    if !config.enabled {
        return submission_errors(
            title_block,
            "The Vibe Kanban Slack integration was disabled before this issue was created.",
        );
    }

    let metadata: ModalMetadata =
        serde_json::from_str(&submission.view.private_metadata).unwrap_or_default();

    let Some(project_id) = state_values
        .selected_option("project", "project_select")
        .and_then(|v| Uuid::parse_str(&v).ok())
    else {
        return submission_errors("project", "Select a project.");
    };
    if title.is_empty() {
        return submission_errors(title_block, "Enter a title.");
    }

    // The project must belong to the connected org — the modal only offers
    // org projects, but the submission payload is client-controlled beyond
    // the signature, so re-check rather than trust it.
    let project = match ProjectRepository::find_by_id(&state.pool, project_id).await {
        Ok(Some(project)) if project.organization_id == config.organization_id => project,
        Ok(_) => {
            return submission_errors("project", "This project no longer exists.");
        }
        Err(error) => {
            tracing::error!(?error, "failed to load project for slack submission");
            return submission_errors("project", "Failed to load the project. Try again.");
        }
    };

    let Some(creator_user_id) = config.created_by_user_id else {
        return submission_errors(
            title_block,
            "The Slack connection has no owning admin (they may have left). \
             An organization admin must re-save the Slack settings in Vibe Kanban.",
        );
    };

    // Idempotency: a replay of this signed submission (Slack retry or an
    // on-path replay inside the timestamp window) carries the same view id;
    // if that modal instance already produced an issue, ack without
    // creating a duplicate.
    let view_id = submission
        .view
        .id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string);
    if let Some(view_id) = view_id.as_deref() {
        match SlackConfigRepository::find_issue_id_by_slack_view(&state.pool, project_id, view_id)
            .await
        {
            Ok(Some(existing)) => {
                tracing::info!(issue_id = %existing, "slack view_submission replay ignored");
                return StatusCode::OK.into_response();
            }
            Ok(None) => {}
            Err(error) => {
                tracing::error!(?error, "failed idempotency lookup for slack submission");
                return submission_errors(title_block, "Failed to create the issue. Try again.");
            }
        }
    }

    // Same initial status a newly created board issue gets: first visible
    // column by sort order.
    let statuses = match ProjectStatusRepository::list_by_project(&state.pool, project_id).await {
        Ok(statuses) => statuses,
        Err(error) => {
            tracing::error!(?error, "failed to load project statuses");
            return submission_errors("project", "Failed to load the project's statuses.");
        }
    };
    let mut sorted = statuses;
    sorted.sort_by_key(|s| s.sort_order);
    let Some(status_id) = sorted
        .iter()
        .find(|s| !s.hidden)
        .or(sorted.first())
        .map(|s| s.id)
    else {
        return submission_errors("project", "This project has no status columns.");
    };

    let sort_order = match SlackConfigRepository::next_sort_order(&state.pool, project_id).await {
        Ok(sort_order) => sort_order,
        Err(error) => {
            tracing::error!(?error, "failed to compute sort order");
            return submission_errors(title_block, "Failed to create the issue. Try again.");
        }
    };

    let extension_metadata = json!({
        "slack": {
            "team_id": metadata.team_id,
            "team_domain": metadata.team_domain,
            "channel_id": metadata.channel_id,
            "message_ts": metadata.message_ts,
            "permalink": metadata.permalink,
            "user_id": metadata.slack_user_id,
            "user_name": metadata.slack_user_name,
            // Idempotency key for replayed submissions (see lookup above).
            "view_id": view_id,
        }
    });

    let created = match IssueRepository::create(
        &state.pool,
        None,
        project_id,
        status_id,
        title,
        description,
        None,
        None,
        None,
        None,
        sort_order,
        None,
        None,
        extension_metadata,
        creator_user_id,
    )
    .await
    {
        Ok(created) => created.data,
        Err(error) => {
            // A unique-violation on idx_issues_slack_view_id means a
            // concurrent replay of this submission won the race — that is
            // success, not failure. Re-check rather than parse error codes
            // across the repository's error type.
            if let Some(view_id) = view_id.as_deref()
                && let Ok(Some(existing)) = SlackConfigRepository::find_issue_id_by_slack_view(
                    &state.pool,
                    project_id,
                    view_id,
                )
                .await
            {
                tracing::info!(issue_id = %existing, "slack view_submission replay lost race");
                return StatusCode::OK.into_response();
            }
            tracing::error!(?error, "failed to create issue from slack");
            return submission_errors(title_block, "Failed to create the issue. Try again.");
        }
    };

    // Ack (closes the modal) and confirm out-of-band: the issue exists, so
    // a failed confirmation is a delivery problem, not a lost creation.
    let issue_url = format!(
        "{}/projects/{}/issues/{}",
        state.server_public_base_url.trim_end_matches('/'),
        created.project_id,
        created.id
    );
    let confirmation = format!(
        "Created <{}|{}: {}> in *{}*",
        issue_url,
        created.simple_id,
        escape_mrkdwn(&created.title),
        escape_mrkdwn(&project.name),
    );
    if let Ok(bot_token) = state.jwt.decrypt_string(&config.encrypted_bot_token) {
        let client = SlackClient::new(state.http_client.clone(), bot_token);
        let channel_id = metadata.channel_id.clone();
        let user_id = metadata.slack_user_id.clone();
        tokio::spawn(async move {
            send_confirmation(&client, &channel_id, &user_id, &confirmation).await;
        });
    }

    StatusCode::OK.into_response()
}

/// Ephemeral in the invoking channel; falls back to a DM when the bot
/// cannot post there (FR-6).
async fn send_confirmation(client: &SlackClient, channel_id: &str, user_id: &str, text: &str) {
    if channel_id.is_empty() || user_id.is_empty() {
        warn!("slack confirmation skipped: missing channel/user in modal metadata");
        return;
    }
    match client.post_ephemeral(channel_id, user_id, text).await {
        Ok(()) => {}
        Err(error) if error.is_channel_access_error() => {
            let result = match client.open_dm(user_id).await {
                Ok(dm_channel) => client.post_message(&dm_channel, text).await,
                Err(error) => Err(error),
            };
            if let Err(error) = result {
                warn!(%error, "failed to deliver slack confirmation via DM fallback");
            }
        }
        Err(error) => {
            warn!(%error, "failed to deliver slack confirmation");
        }
    }
}

/// Escape user text for Slack mrkdwn (link labels, bold spans).
fn escape_mrkdwn(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_payload_from_form_body() {
        let body = b"payload=%7B%22type%22%3A%22message_action%22%7D";
        assert_eq!(
            extract_payload_field(body).as_deref(),
            Some(r#"{"type":"message_action"}"#)
        );
    }

    #[test]
    fn missing_payload_field_is_none() {
        assert!(extract_payload_field(b"other=1").is_none());
        assert!(extract_payload_field(b"").is_none());
    }

    #[test]
    fn escapes_mrkdwn_control_characters() {
        assert_eq!(escape_mrkdwn("a <b> & c"), "a &lt;b&gt; &amp; c");
    }
}
