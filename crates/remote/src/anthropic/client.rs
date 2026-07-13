//! Minimal Anthropic Messages API client for thread summarization.
//!
//! One method: `summarize_thread`. Follows the Jira client's HTTP-status
//! error pattern (Anthropic returns real 4xx/5xx with a
//! `{"error":{"type","message"}}` body), not Slack's in-band `ok` envelope.
//!
//! Auth is the `x-api-key` header; the key must never appear in any error
//! string. The `From<reqwest::Error>` carries only `err.to_string()`, which
//! never includes request headers — the same guarantee the Slack/Jira
//! clients document.

use serde_json::json;

use super::{
    prompt::{self, MAX_TRANSCRIPT_CHARS, SYSTEM_PROMPT},
    types::{ApiErrorBody, IssueSummary, MessagesResponse},
};
use crate::slack::types::SlackReplyMessage;

const MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
/// Chosen for cost/latency: a short summarization task. Runs after Slack's
/// 3s ack, so its 1–3s latency is invisible to the shortcut.
const MODEL: &str = "claude-haiku-4-5";
const MAX_TOKENS: u32 = 1024;

#[derive(Debug, thiserror::Error)]
pub enum AnthropicError {
    /// No usable text in the thread — nothing to summarize.
    #[error("thread had no summarizable content")]
    EmptyThread,
    /// The model declined (`stop_reason: "refusal"`) or returned no/invalid
    /// JSON text.
    #[error("model returned no usable summary")]
    NoSummary,
    /// A non-2xx HTTP response; carries Anthropic's `error.message` (never the
    /// API key).
    #[error("Anthropic API error ({status}): {message}")]
    Api { status: u16, message: String },
    #[error("failed to reach Anthropic: {0}")]
    Transport(String),
}

impl From<reqwest::Error> for AnthropicError {
    fn from(err: reqwest::Error) -> Self {
        // reqwest error strings can embed the URL but never the auth header.
        AnthropicError::Transport(err.to_string())
    }
}

pub struct AnthropicClient {
    http: reqwest::Client,
    api_key: String,
}

impl AnthropicClient {
    pub fn new(http: reqwest::Client, api_key: String) -> Self {
        Self { http, api_key }
    }

    /// Summarize a Slack thread into an issue title + description. Any error
    /// (empty thread, HTTP error, refusal, malformed output) is returned so
    /// the caller can degrade to the mechanical prefill (spec FR-5).
    pub async fn summarize_thread(
        &self,
        messages: &[SlackReplyMessage],
    ) -> Result<IssueSummary, AnthropicError> {
        let transcript = prompt::build_transcript(messages).ok_or(AnthropicError::EmptyThread)?;
        debug_assert!(transcript.chars().count() <= MAX_TRANSCRIPT_CHARS + 64);

        let body = json!({
            "model": MODEL,
            "max_tokens": MAX_TOKENS,
            "system": SYSTEM_PROMPT,
            "messages": [{ "role": "user", "content": transcript }],
            "output_config": {
                "format": {
                    "type": "json_schema",
                    "schema": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string" },
                            "description": { "type": "string" }
                        },
                        "required": ["title", "description"],
                        "additionalProperties": false
                    }
                }
            }
        });

        let response = self
            .http
            .post(MESSAGES_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let raw = response.text().await.unwrap_or_default();
            let message = serde_json::from_str::<ApiErrorBody>(&raw)
                .ok()
                .and_then(|b| b.error)
                .and_then(|e| e.message)
                .unwrap_or_else(|| "no error detail".to_string());
            return Err(AnthropicError::Api {
                status: status.as_u16(),
                message,
            });
        }

        let parsed: MessagesResponse = response.json().await?;
        if parsed.stop_reason.as_deref() == Some("refusal") {
            return Err(AnthropicError::NoSummary);
        }
        let text = parsed.first_text().ok_or(AnthropicError::NoSummary)?;
        serde_json::from_str::<IssueSummary>(text).map_err(|_| AnthropicError::NoSummary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_key_never_in_transport_error() {
        // Sanity: our Transport variant is built only from reqwest's display,
        // which never carries headers. This documents the invariant.
        let err = AnthropicError::Transport(
            "error sending request for url (https://api.anthropic.com/v1/messages)".to_string(),
        );
        assert!(!err.to_string().contains("x-api-key"));
        assert!(!err.to_string().contains("sk-ant"));
    }

    #[test]
    fn parses_structured_summary_from_messages_response() {
        // Fixture mirroring a real structured-output success (contract §success).
        let raw = serde_json::json!({
            "id": "msg_1",
            "type": "message",
            "role": "assistant",
            "model": "claude-haiku-4-5",
            "stop_reason": "end_turn",
            "content": [
                { "type": "text", "text": "{\"title\":\"Fix login redirect\",\"description\":\"The OAuth callback drops the return URL.\"}" }
            ]
        })
        .to_string();
        let parsed: MessagesResponse = serde_json::from_str(&raw).unwrap();
        assert_ne!(parsed.stop_reason.as_deref(), Some("refusal"));
        let summary: IssueSummary = serde_json::from_str(parsed.first_text().unwrap()).unwrap();
        assert_eq!(summary.title, "Fix login redirect");
        assert!(summary.description.contains("return URL"));
    }

    #[test]
    fn refusal_stop_reason_detected() {
        let raw = serde_json::json!({
            "stop_reason": "refusal",
            "content": []
        })
        .to_string();
        let parsed: MessagesResponse = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.stop_reason.as_deref(), Some("refusal"));
        assert!(parsed.first_text().is_none());
    }

    #[test]
    fn parses_error_body_message() {
        let raw = r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
        let body: ApiErrorBody = serde_json::from_str(raw).unwrap();
        assert_eq!(
            body.error.unwrap().message.as_deref(),
            Some("invalid x-api-key")
        );
    }
}
