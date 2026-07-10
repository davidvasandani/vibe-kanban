//! Bidirectional Jira <-> VK project sync.
//!
//! One Jira connection per project, configured in project settings: issues
//! matching a JQL query are mirrored as VK issues, and synced-field changes
//! (title, description, status) flow both ways via a periodic reconciler.

pub mod client;
pub mod mapping;
pub mod merge;
pub mod sync;
pub mod types;
