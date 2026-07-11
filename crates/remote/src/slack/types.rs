//! Types for the Slack integration: the DB config row, inbound interaction
//! payloads (tolerant serde views of what Slack sends), and the REST DTOs
//! exported to TypeScript.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Full config row. Internal only — carries ciphertext and is never
/// serialized to clients (see [`SlackConfigResponse`]).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SlackConfig {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub encrypted_bot_token: String,
    pub encrypted_signing_secret: String,
    pub slack_team_id: String,
    pub slack_team_name: String,
    pub enabled: bool,
    pub created_by_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Client-facing view of the config: credentials replaced by
/// `has_credentials`. `interactivity_url` is the request URL the workspace
/// admin pastes into the Slack app manifest.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SlackConfigResponse {
    pub organization_id: Uuid,
    pub slack_team_id: String,
    pub slack_team_name: String,
    pub enabled: bool,
    pub has_credentials: bool,
    pub interactivity_url: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Upsert request. Credentials are write-only: `None` keeps the stored
/// value (both are required on first save).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpsertSlackConfigRequest {
    pub bot_token: Option<String>,
    pub signing_secret: Option<String>,
    pub enabled: bool,
}

/// Result of `auth.test` against the stored bot token. `error` is Slack's
/// error code (e.g. `invalid_auth`) — never credential material.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SlackTestConnectionResponse {
    pub ok: bool,
    pub team_name: Option<String>,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Inbound interaction payloads (Slack -> us).
// ---------------------------------------------------------------------------

pub const MESSAGE_SHORTCUT_CALLBACK_ID: &str = "vk_create_issue_from_message";
pub const CREATE_ISSUE_MODAL_CALLBACK_ID: &str = "vk_create_issue_modal";

/// Minimal first parse of any interaction payload: just enough to find the
/// config (and its signing secret) and dispatch. Reading this before
/// signature verification is safe — it has no side effects and the payload
/// is dropped unless verification passes.
#[derive(Debug, Deserialize)]
pub struct InteractionPeek {
    #[serde(rename = "type")]
    pub kind: String,
    pub team: Option<SlackTeamRef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlackTeamRef {
    pub id: String,
    #[serde(default)]
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlackUserRef {
    pub id: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

impl SlackUserRef {
    /// Best display name Slack gives us in interaction payloads.
    pub fn display_name(&self) -> Option<String> {
        self.username.clone().or_else(|| self.name.clone())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlackChannelRef {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlackMessageRef {
    #[serde(default)]
    pub ts: Option<String>,
    /// Absent in contexts where Slack does not deliver message content;
    /// the modal then opens with empty prefills (FR-8).
    #[serde(default)]
    pub text: Option<String>,
}

/// `type: message_action` — the user ran the message shortcut.
#[derive(Debug, Deserialize)]
pub struct MessageActionPayload {
    pub callback_id: String,
    pub trigger_id: String,
    pub team: SlackTeamRef,
    pub user: SlackUserRef,
    pub channel: SlackChannelRef,
    pub message: SlackMessageRef,
}

/// `type: view_submission` — the user submitted the create-issue modal.
#[derive(Debug, Deserialize)]
pub struct ViewSubmissionPayload {
    pub user: SlackUserRef,
    pub view: SlackView,
}

#[derive(Debug, Deserialize)]
pub struct SlackView {
    /// Slack's unique id for this modal instance — the idempotency key for
    /// submissions (a replayed `view_submission` carries the same view id).
    #[serde(default)]
    pub id: Option<String>,
    pub callback_id: String,
    #[serde(default)]
    pub private_metadata: String,
    pub state: SlackViewState,
}

#[derive(Debug, Deserialize)]
pub struct SlackViewState {
    pub values: serde_json::Value,
}

impl SlackViewState {
    /// Fetch `state.values[block_id][action_id]`.
    fn action(&self, block_id: &str, action_id: &str) -> Option<&serde_json::Value> {
        self.values.get(block_id)?.get(action_id)
    }

    pub fn text_input(&self, block_id: &str, action_id: &str) -> Option<String> {
        self.action(block_id, action_id)?
            .get("value")?
            .as_str()
            .map(str::to_string)
    }

    pub fn selected_option(&self, block_id: &str, action_id: &str) -> Option<String> {
        self.action(block_id, action_id)?
            .get("selected_option")?
            .get("value")?
            .as_str()
            .map(str::to_string)
    }
}

/// Context carried through the modal round trip in `view.private_metadata`
/// (Slack echoes it back on submission, 3000-char limit — ids only, never
/// message text). The submission is independently signature-verified, so
/// trusting this does not extend trust beyond the signing secret.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModalMetadata {
    pub team_id: String,
    #[serde(default)]
    pub team_domain: Option<String>,
    pub channel_id: String,
    #[serde(default)]
    pub message_ts: Option<String>,
    #[serde(default)]
    pub permalink: Option<String>,
    pub slack_user_id: String,
    #[serde(default)]
    pub slack_user_name: Option<String>,
}

// ---------------------------------------------------------------------------
// Slack Web API responses (us -> Slack).
// ---------------------------------------------------------------------------

/// Envelope every Web API method returns: `{ok: bool, error?: string, ...}`.
#[derive(Debug, Deserialize)]
pub struct SlackApiEnvelope {
    pub ok: bool,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SlackAuthTestResponse {
    pub ok: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub team: Option<String>,
    #[serde(default)]
    pub team_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SlackConversationsOpenResponse {
    pub ok: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub channel: Option<SlackChannelRef>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_message_action_payload() {
        let payload = serde_json::json!({
            "type": "message_action",
            "callback_id": "vk_create_issue_from_message",
            "trigger_id": "12345.98765.abcd",
            "team": {"id": "T0123", "domain": "acme"},
            "user": {"id": "U0123", "username": "dvasandani", "name": "david"},
            "channel": {"id": "C0123", "name": "eng"},
            "message": {"type": "message", "ts": "1720600000.123456", "text": "fix the login bug"},
            "response_url": "https://hooks.slack.com/app/T0123/123/xyz"
        })
        .to_string();

        let peek: InteractionPeek = serde_json::from_str(&payload).unwrap();
        assert_eq!(peek.kind, "message_action");
        assert_eq!(peek.team.unwrap().id, "T0123");

        let action: MessageActionPayload = serde_json::from_str(&payload).unwrap();
        assert_eq!(action.callback_id, MESSAGE_SHORTCUT_CALLBACK_ID);
        assert_eq!(action.team.domain.as_deref(), Some("acme"));
        assert_eq!(action.message.text.as_deref(), Some("fix the login bug"));
        assert_eq!(action.user.display_name().as_deref(), Some("dvasandani"));
    }

    #[test]
    fn parses_message_action_without_text() {
        // Restricted contexts can omit message content (FR-8).
        let payload = serde_json::json!({
            "type": "message_action",
            "callback_id": "vk_create_issue_from_message",
            "trigger_id": "t",
            "team": {"id": "T0123"},
            "user": {"id": "U0123"},
            "channel": {"id": "C0123"},
            "message": {}
        })
        .to_string();

        let action: MessageActionPayload = serde_json::from_str(&payload).unwrap();
        assert!(action.message.text.is_none());
        assert!(action.team.domain.is_none());
    }

    #[test]
    fn parses_view_submission_state_values() {
        let payload = serde_json::json!({
            "type": "view_submission",
            "team": {"id": "T0123"},
            "user": {"id": "U0123", "username": "dvasandani"},
            "view": {
                "callback_id": "vk_create_issue_modal",
                "private_metadata": "{\"team_id\":\"T0123\",\"channel_id\":\"C0123\",\"slack_user_id\":\"U0123\"}",
                "state": {"values": {
                    "project": {"project_select": {"type": "static_select",
                        "selected_option": {"value": "3fa85f64-5717-4562-b3fc-2c963f66afa6"}}},
                    "title": {"title_input": {"type": "plain_text_input", "value": "Fix login"}},
                    "description": {"description_input": {"type": "plain_text_input", "value": "Details"}}
                }}
            }
        })
        .to_string();

        let submission: ViewSubmissionPayload = serde_json::from_str(&payload).unwrap();
        assert_eq!(submission.view.callback_id, CREATE_ISSUE_MODAL_CALLBACK_ID);
        assert_eq!(
            submission
                .view
                .state
                .selected_option("project", "project_select")
                .as_deref(),
            Some("3fa85f64-5717-4562-b3fc-2c963f66afa6")
        );
        assert_eq!(
            submission
                .view
                .state
                .text_input("title", "title_input")
                .as_deref(),
            Some("Fix login")
        );

        let metadata: ModalMetadata =
            serde_json::from_str(&submission.view.private_metadata).unwrap();
        assert_eq!(metadata.channel_id, "C0123");
        assert!(metadata.permalink.is_none());
    }
}
