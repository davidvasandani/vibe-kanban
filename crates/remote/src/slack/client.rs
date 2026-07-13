//! Minimal Slack Web API client.
//!
//! All methods are `POST https://slack.com/api/{method}` with a bearer bot
//! token. Error strings must never contain the token; auth material only
//! ever goes into request headers.

use serde_json::{Value, json};

use super::types::{
    SlackApiEnvelope, SlackAuthTestResponse, SlackConversationsOpenResponse,
    SlackConversationsRepliesResponse, SlackReplyMessage, SlackViewsOpenResponse,
};

const SLACK_API_BASE: &str = "https://slack.com/api";

#[derive(Debug, thiserror::Error)]
pub enum SlackClientError {
    /// Slack answered `ok: false`; carries Slack's error code
    /// (e.g. `invalid_auth`, `channel_not_found`).
    #[error("Slack API error: {0}")]
    Api(String),
    #[error("failed to reach Slack: {0}")]
    Transport(String),
}

impl From<reqwest::Error> for SlackClientError {
    fn from(err: reqwest::Error) -> Self {
        // reqwest errors can embed the URL (never the auth header); the
        // display string is safe to surface.
        SlackClientError::Transport(err.to_string())
    }
}

impl SlackClientError {
    /// Errors meaning "can't post in that channel" — the cue to fall back
    /// from an ephemeral channel message to a DM (FR-6).
    pub fn is_channel_access_error(&self) -> bool {
        matches!(
            self,
            SlackClientError::Api(code) if matches!(
                code.as_str(),
                "channel_not_found" | "not_in_channel" | "user_not_in_channel" | "is_archived"
            )
        )
    }
}

pub struct SlackClient {
    http: reqwest::Client,
    bot_token: String,
}

impl SlackClient {
    pub fn new(http: reqwest::Client, bot_token: String) -> Self {
        Self { http, bot_token }
    }

    async fn call(&self, method: &str, body: Value) -> Result<Value, SlackClientError> {
        let response = self
            .http
            .post(format!("{SLACK_API_BASE}/{method}"))
            .bearer_auth(&self.bot_token)
            .json(&body)
            .send()
            .await?;
        let value: Value = response.json().await?;
        let envelope: SlackApiEnvelope = serde_json::from_value(value.clone()).map_err(|_| {
            SlackClientError::Transport("Slack returned an unexpected response".to_string())
        })?;
        if !envelope.ok {
            return Err(SlackClientError::Api(
                envelope
                    .error
                    .unwrap_or_else(|| "unknown_error".to_string()),
            ));
        }
        Ok(value)
    }

    /// Validate the token and identify the workspace it belongs to.
    pub async fn auth_test(&self) -> Result<SlackAuthTestResponse, SlackClientError> {
        let value = self.call("auth.test", json!({})).await?;
        serde_json::from_value(value).map_err(|_| {
            SlackClientError::Transport("Slack returned an unexpected response".to_string())
        })
    }

    /// Open a modal from an interaction's `trigger_id` (valid for 3s).
    /// Returns the created view's id so a follow-up `views_update` can swap in
    /// the AI summary (FR-1/FR-8); `None` if Slack omits it.
    pub async fn views_open(
        &self,
        trigger_id: &str,
        view: Value,
    ) -> Result<Option<String>, SlackClientError> {
        let value = self
            .call(
                "views.open",
                json!({"trigger_id": trigger_id, "view": view}),
            )
            .await?;
        let response: SlackViewsOpenResponse = serde_json::from_value(value).map_err(|_| {
            SlackClientError::Transport("Slack returned an unexpected response".to_string())
        })?;
        Ok(response.view.map(|v| v.id))
    }

    /// Replace an already-open modal by its view id (`views.update`). Used to
    /// swap the mechanical prefill for the AI summary, or to drop the
    /// "Summarizing…" hint on the AI-failure path.
    pub async fn views_update(&self, view_id: &str, view: Value) -> Result<(), SlackClientError> {
        self.call("views.update", json!({"view_id": view_id, "view": view}))
            .await?;
        Ok(())
    }

    /// Fetch the replies of a thread (root at index 0), for AI summarization.
    /// Requires a message-history read scope (`channels:history` etc.); a
    /// missing scope surfaces as `Api("missing_scope")` and the caller falls
    /// back to the mechanical prefill (FR-5/FR-13).
    pub async fn conversations_replies(
        &self,
        channel_id: &str,
        thread_ts: &str,
        limit: usize,
    ) -> Result<Vec<SlackReplyMessage>, SlackClientError> {
        let value = self
            .call(
                "conversations.replies",
                json!({"channel": channel_id, "ts": thread_ts, "limit": limit}),
            )
            .await?;
        let response: SlackConversationsRepliesResponse =
            serde_json::from_value(value).map_err(|_| {
                SlackClientError::Transport("Slack returned an unexpected response".to_string())
            })?;
        Ok(response.messages)
    }

    /// Post a message only the given user can see, in the given channel.
    pub async fn post_ephemeral(
        &self,
        channel_id: &str,
        user_id: &str,
        text: &str,
    ) -> Result<(), SlackClientError> {
        self.call(
            "chat.postEphemeral",
            json!({"channel": channel_id, "user": user_id, "text": text}),
        )
        .await?;
        Ok(())
    }

    /// Open (or resume) a DM with the user; returns the DM channel id.
    pub async fn open_dm(&self, user_id: &str) -> Result<String, SlackClientError> {
        let value = self
            .call("conversations.open", json!({"users": user_id}))
            .await?;
        let response: SlackConversationsOpenResponse =
            serde_json::from_value(value).map_err(|_| {
                SlackClientError::Transport("Slack returned an unexpected response".to_string())
            })?;
        response
            .channel
            .map(|c| c.id)
            .ok_or_else(|| SlackClientError::Api("no_channel_in_response".to_string()))
    }

    pub async fn post_message(&self, channel_id: &str, text: &str) -> Result<(), SlackClientError> {
        self.call(
            "chat.postMessage",
            json!({"channel": channel_id, "text": text}),
        )
        .await?;
        Ok(())
    }
}
