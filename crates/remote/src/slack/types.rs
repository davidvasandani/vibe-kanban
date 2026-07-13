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
    /// AES-256-GCM ciphertext of the org's Anthropic API key. `None` (the
    /// column is nullable) means AI summarization is not configured.
    pub encrypted_anthropic_api_key: Option<String>,
    pub ai_summarization_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SlackConfig {
    /// AI summarization runs only when the connection is enabled, the toggle
    /// is on, and a key is stored (spec FR-11 — any one false ⇒ mechanical
    /// prefill only, no thread fetch, no Anthropic call).
    pub fn ai_summarization_active(&self) -> bool {
        self.enabled
            && self.ai_summarization_enabled
            && self
                .encrypted_anthropic_api_key
                .as_deref()
                .is_some_and(|k| !k.is_empty())
    }
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
    /// AI summarization toggle (spec FR-9).
    pub ai_summarization_enabled: bool,
    /// True iff an Anthropic API key is stored — the key itself is never
    /// returned (FR-10).
    pub has_anthropic_api_key: bool,
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
    /// AI summarization toggle. `None` keeps the stored value.
    #[serde(default)]
    pub ai_summarization_enabled: Option<bool>,
    /// Anthropic API key. `None`/empty keeps the stored value (write-only,
    /// same semantics as `bot_token`).
    #[serde(default)]
    pub anthropic_api_key: Option<String>,
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
    /// The parent thread's timestamp when this message is a threaded reply;
    /// absent for a standalone message. The AI summary fetches replies of
    /// `thread_ts`, falling back to `ts` so a standalone message summarizes
    /// itself (spec FR-3).
    #[serde(default)]
    pub thread_ts: Option<String>,
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

/// `views.open` response — we keep the created view's id so a later
/// `views.update` can swap in the AI summary (spec FR-1/FR-8).
#[derive(Debug, Deserialize)]
pub struct SlackViewsOpenResponse {
    pub ok: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub view: Option<SlackViewIdRef>,
}

#[derive(Debug, Deserialize)]
pub struct SlackViewIdRef {
    pub id: String,
}

/// `conversations.replies` response — the thread's messages, oldest-first
/// (root at index 0). Used only to build the AI summarization transcript.
/// `conversations.replies` is oldest-first and paginated, so the client walks
/// `response_metadata.next_cursor` to reach the most-recent replies.
#[derive(Debug, Deserialize)]
pub struct SlackConversationsRepliesResponse {
    pub ok: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub messages: Vec<SlackReplyMessage>,
    #[serde(default)]
    pub response_metadata: Option<SlackResponseMetadata>,
}

#[derive(Debug, Deserialize)]
pub struct SlackResponseMetadata {
    #[serde(default)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlackReplyMessage {
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    /// Message timestamp — used to dedup the thread root, which Slack repeats
    /// as the first message of every `conversations.replies` page.
    #[serde(default)]
    pub ts: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with(enabled: bool, ai_enabled: bool, key: Option<&str>) -> SlackConfig {
        SlackConfig {
            id: Uuid::nil(),
            organization_id: Uuid::nil(),
            encrypted_bot_token: "bt".into(),
            encrypted_signing_secret: "ss".into(),
            slack_team_id: "T0".into(),
            slack_team_name: "Acme".into(),
            enabled,
            created_by_user_id: None,
            encrypted_anthropic_api_key: key.map(str::to_string),
            ai_summarization_enabled: ai_enabled,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn ai_summarization_active_requires_all_three() {
        assert!(config_with(true, true, Some("k")).ai_summarization_active());
        // Any one false ⇒ inactive (FR-11).
        assert!(!config_with(false, true, Some("k")).ai_summarization_active());
        assert!(!config_with(true, false, Some("k")).ai_summarization_active());
        assert!(!config_with(true, true, None).ai_summarization_active());
        assert!(!config_with(true, true, Some("")).ai_summarization_active());
    }

    #[test]
    fn parses_conversations_replies_response() {
        let raw = serde_json::json!({
            "ok": true,
            "messages": [
                {"user": "U1", "text": "root", "ts": "1.0001"},
                {"user": "U2", "text": "reply"},
                {"ts": "1.0003"} // no user/text — tolerated
            ]
        })
        .to_string();
        let parsed: SlackConversationsRepliesResponse = serde_json::from_str(&raw).unwrap();
        assert!(parsed.ok);
        assert_eq!(parsed.messages.len(), 3);
        assert_eq!(parsed.messages[0].user.as_deref(), Some("U1"));
        assert_eq!(parsed.messages[2].text, None);

        let err = serde_json::json!({"ok": false, "error": "missing_scope"}).to_string();
        let parsed_err: SlackConversationsRepliesResponse = serde_json::from_str(&err).unwrap();
        assert!(!parsed_err.ok);
        assert_eq!(parsed_err.error.as_deref(), Some("missing_scope"));
    }

    #[test]
    fn parses_thread_ts_and_views_open_view_id() {
        let action = serde_json::json!({
            "type": "message_action",
            "callback_id": "vk_create_issue_from_message",
            "trigger_id": "t",
            "team": {"id": "T0"},
            "user": {"id": "U0"},
            "channel": {"id": "C0"},
            "message": {"ts": "2.0", "thread_ts": "1.0", "text": "reply in thread"}
        })
        .to_string();
        let parsed: MessageActionPayload = serde_json::from_str(&action).unwrap();
        assert_eq!(parsed.message.thread_ts.as_deref(), Some("1.0"));

        let open = serde_json::json!({"ok": true, "view": {"id": "V123"}}).to_string();
        let parsed_open: SlackViewsOpenResponse = serde_json::from_str(&open).unwrap();
        assert_eq!(parsed_open.view.map(|v| v.id).as_deref(), Some("V123"));
    }

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
