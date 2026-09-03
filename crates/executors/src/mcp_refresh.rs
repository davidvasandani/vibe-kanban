use std::{fmt, sync::Arc};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum McpRefreshStatus {
    PendingNextTurn,
    Refreshed,
    PartiallyRefreshed,
    Busy,
    Unsupported,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum McpRefreshErrorCategory {
    ExecutableUnavailable,
    ProcessLaunchFailed,
    InitializeFailed,
    AuthenticationFailed,
    CapabilityListFailed,
    InvalidCapabilitySchema,
    Timeout,
    RefreshInProgress,
    ActiveCall,
    MaterializationFailed,
    ReloadFailed,
    Unsupported,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS, JsonSchema)]
#[ts(export)]
pub struct McpRefreshError {
    pub category: McpRefreshErrorCategory,
    pub message: String,
    pub remediation: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum McpServerRefreshStatus {
    Ready,
    FailedRetained,
    FailedUnavailable,
    Removed,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS, JsonSchema)]
#[ts(export)]
pub struct McpServerRefreshSnapshot {
    pub server_id: String,
    pub status: McpServerRefreshStatus,
    pub tool_count: Option<u32>,
    /// Sorted tool identifiers from the executor-owned, post-start inventory.
    pub tool_names: Option<Vec<String>>,
    /// SHA-256 of sorted tool identifiers and their input/output schemas.
    pub tool_schema_fingerprint: Option<String>,
    pub resource_count: Option<u32>,
    pub prompt_count: Option<u32>,
    pub restart_occurred: Option<bool>,
    pub error: Option<McpRefreshError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS, JsonSchema)]
#[ts(export)]
pub struct McpRefreshResult {
    pub status: McpRefreshStatus,
    pub retryable: bool,
    pub generation: u64,
    pub requested_at: DateTime<Utc>,
    pub last_successful_refresh_at: Option<DateTime<Utc>>,
    pub servers: Vec<McpServerRefreshSnapshot>,
    pub error: Option<McpRefreshError>,
}

#[async_trait]
pub trait McpRefreshControl: Send + Sync {
    async fn queue_refresh(&self) -> Result<(), McpRefreshErrorCategory>;
    async fn list_servers(&self) -> Result<Vec<McpServerRefreshSnapshot>, McpRefreshErrorCategory>;
}

#[derive(Clone)]
pub struct McpRefreshHandle(pub Arc<dyn McpRefreshControl>);

impl fmt::Debug for McpRefreshHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("McpRefreshHandle")
            .field(&"<live executor control>")
            .finish()
    }
}

pub type McpRefreshSignal = tokio::sync::oneshot::Receiver<McpRefreshHandle>;

pub fn safe_executor_error(category: McpRefreshErrorCategory) -> McpRefreshError {
    let (message, remediation, retryable) = match category {
        McpRefreshErrorCategory::AuthenticationFailed => (
            "The MCP server requires authentication.",
            "Reconnect or renew the server credentials, then refresh again.",
            true,
        ),
        McpRefreshErrorCategory::Timeout => (
            "The MCP refresh timed out.",
            "Check that the server is responsive, then retry.",
            true,
        ),
        McpRefreshErrorCategory::RefreshInProgress => (
            "An MCP refresh is already in progress.",
            "Wait for the current refresh to finish, then retry.",
            true,
        ),
        McpRefreshErrorCategory::ActiveCall => (
            "An MCP tool call is active.",
            "Retry after the active tool call finishes.",
            true,
        ),
        McpRefreshErrorCategory::MaterializationFailed => (
            "Vibe Kanban could not materialize the latest MCP settings.",
            "Retry. If the problem continues, inspect the worker's secret-safe logs.",
            true,
        ),
        McpRefreshErrorCategory::ReloadFailed => (
            "Codex could not reload the refreshed MCP configuration.",
            "Retry after Codex is ready, or continue in a fresh turn.",
            true,
        ),
        McpRefreshErrorCategory::Unsupported => (
            "This executor cannot refresh MCP tools in place.",
            "Start the next turn with an executor that supports live MCP refresh.",
            false,
        ),
        McpRefreshErrorCategory::ExecutableUnavailable => (
            "The configured MCP executable or package is unavailable.",
            "Install the configured package or correct its command, then retry.",
            true,
        ),
        McpRefreshErrorCategory::ProcessLaunchFailed => (
            "The MCP server could not be started.",
            "Check the server command and local runtime, then retry.",
            true,
        ),
        McpRefreshErrorCategory::InitializeFailed => (
            "The MCP server did not complete initialization.",
            "Check server compatibility and startup health, then retry.",
            true,
        ),
        McpRefreshErrorCategory::CapabilityListFailed => (
            "The refreshed MCP capability inventory could not be read.",
            "Check the server logs for a tools/list failure, then retry.",
            true,
        ),
        McpRefreshErrorCategory::InvalidCapabilitySchema => (
            "The MCP server returned an invalid capability schema.",
            "Update or fix the MCP server, then retry.",
            true,
        ),
        McpRefreshErrorCategory::Internal => (
            "Vibe Kanban could not complete the MCP refresh.",
            "Retry. If the problem continues, inspect secret-safe server logs.",
            true,
        ),
    };
    McpRefreshError {
        category,
        message: message.to_string(),
        remediation: remediation.to_string(),
        retryable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_errors_are_allow_listed_and_secret_free() {
        let secret = "token-super-secret";
        for category in [
            McpRefreshErrorCategory::ExecutableUnavailable,
            McpRefreshErrorCategory::ProcessLaunchFailed,
            McpRefreshErrorCategory::InitializeFailed,
            McpRefreshErrorCategory::AuthenticationFailed,
            McpRefreshErrorCategory::CapabilityListFailed,
            McpRefreshErrorCategory::InvalidCapabilitySchema,
            McpRefreshErrorCategory::Timeout,
            McpRefreshErrorCategory::RefreshInProgress,
            McpRefreshErrorCategory::ActiveCall,
            McpRefreshErrorCategory::MaterializationFailed,
            McpRefreshErrorCategory::ReloadFailed,
            McpRefreshErrorCategory::Unsupported,
            McpRefreshErrorCategory::Internal,
        ] {
            let encoded = serde_json::to_string(&safe_executor_error(category)).unwrap();
            assert!(!encoded.contains(secret));
            assert!(!encoded.contains("http://"));
            assert!(!encoded.contains("https://"));
        }
    }
}
