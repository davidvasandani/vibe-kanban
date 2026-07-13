//! Serde types for the Anthropic Messages API call and its result.
//!
//! Only the fields we send/read are modeled; the response is otherwise
//! tolerated. `IssueSummary` is the structured output we force via
//! `output_config.format` and the only thing that leaves this module.

use serde::{Deserialize, Serialize};

/// The structured summary the model returns (schema-constrained to exactly
/// these two fields). Becomes the modal's title/description prefill.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IssueSummary {
    pub title: String,
    pub description: String,
}

/// A `content` block in the Messages API response. Structured outputs
/// guarantee the first block is `type: "text"` with JSON text.
#[derive(Debug, Deserialize)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub text: Option<String>,
}

/// The Messages API success envelope (the parts we read).
#[derive(Debug, Deserialize)]
pub struct MessagesResponse {
    #[serde(default)]
    pub content: Vec<ContentBlock>,
    /// `"end_turn"`, `"max_tokens"`, `"refusal"`, … A refusal is treated as a
    /// summarization failure (degrade to the mechanical prefill).
    #[serde(default)]
    pub stop_reason: Option<String>,
}

impl MessagesResponse {
    /// The first `text` block's content, if any.
    pub fn first_text(&self) -> Option<&str> {
        self.content
            .iter()
            .find(|b| b.kind == "text")
            .and_then(|b| b.text.as_deref())
    }
}

/// The `{"type":"error","error":{"type","message"}}` body Anthropic returns
/// on non-2xx. `message` never echoes the request headers (the API key).
#[derive(Debug, Deserialize)]
pub struct ApiErrorBody {
    #[serde(default)]
    pub error: Option<ApiErrorDetail>,
}

#[derive(Debug, Deserialize)]
pub struct ApiErrorDetail {
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}
