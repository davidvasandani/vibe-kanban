//! Slack app integration: the "Create issue from message" shortcut.
//!
//! One inbound endpoint (`POST /v1/slack/interactivity`, see
//! `routes::slack`) receives Slack interaction payloads; everything else is
//! outbound Web API calls made with the org's bot token. Credentials are
//! stored encrypted per organization (`organization_slack_configs`) and are
//! write-only over the REST API.

pub mod client;
pub mod modal;
pub mod prefill;
pub mod signature;
pub mod types;
