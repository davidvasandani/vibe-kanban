//! Anthropic Messages API integration.
//!
//! Used only by the Slack shortcut's optional AI summarization: turn a Slack
//! thread into an issue title + description via `claude-haiku-4-5`. Outbound
//! only, over the shared HTTP client; the org's API key is a write-only
//! encrypted credential. Every failure degrades to the mechanical prefill
//! (spec FR-5, constitution: outbound AI/LLM egress is opt-in and degradable).

pub mod client;
pub mod prompt;
pub mod types;
