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

/// Cheap to clone — `reqwest::Client` is `Arc`-backed and shares the pool; the
/// bot token is a short `String`. Cloning lets the summarizing animation
/// fire-and-forget per-frame `views.update` calls off the select loop.
#[derive(Clone)]
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

    /// Fetch a thread's messages (root at index 0), oldest-first, for AI
    /// summarization. `conversations.replies` is oldest-first and paginated, so
    /// we walk `response_metadata.next_cursor` up to `max_pages` to reach the
    /// most-recent replies (the summarizer keeps root + newest). The thread
    /// root is repeated as the first message of every page, so we dedup by
    /// message `ts`. Requires a message-history read scope
    /// (`channels:history` etc.); a missing scope surfaces as
    /// `Api("missing_scope")` and the caller falls back to the mechanical
    /// prefill (FR-5/FR-13).
    pub async fn conversations_replies(
        &self,
        channel_id: &str,
        thread_ts: &str,
        per_page: usize,
        max_pages: usize,
    ) -> Result<Vec<SlackReplyMessage>, SlackClientError> {
        let mut all: Vec<SlackReplyMessage> = Vec::new();
        let mut seen_ts: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut cursor: Option<String> = None;

        for _ in 0..max_pages.max(1) {
            let mut body = json!({"channel": channel_id, "ts": thread_ts, "limit": per_page});
            if let Some(c) = &cursor {
                body["cursor"] = json!(c);
            }
            let value = self.call("conversations.replies", body).await?;
            let response: SlackConversationsRepliesResponse = serde_json::from_value(value)
                .map_err(|_| {
                    SlackClientError::Transport("Slack returned an unexpected response".to_string())
                })?;

            for message in response.messages {
                // Dedup the repeated root by ts; messages without a ts (rare)
                // are kept as-is.
                match &message.ts {
                    Some(ts) if !seen_ts.insert(ts.clone()) => continue,
                    _ => all.push(message),
                }
            }

            match response
                .response_metadata
                .and_then(|m| m.next_cursor)
                .filter(|c| !c.is_empty())
            {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        Ok(all)
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
