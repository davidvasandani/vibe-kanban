use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::mcp_refresh::{McpRefreshError, McpServerRefreshSnapshot};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum McpRecoveryScope {
    Session,
    Workspace,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum McpRecoveryStatus {
    Accepted,
    InProgress,
    Completed,
    PartiallyCompleted,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum McpRestartDisposition {
    Queued,
    Started,
    AlreadyInProgress,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS, JsonSchema)]
#[ts(export)]
/// Recovery state including the executor-owned registry snapshot. An empty
/// server vector means the replacement process has not published a registry
/// yet, never that discovery succeeded.
pub struct McpRecoveryResult {
    pub generation: u64,
    pub scope: McpRecoveryScope,
    pub workspace_id: Uuid,
    pub session_id: Option<Uuid>,
    pub status: McpRecoveryStatus,
    pub disposition: McpRestartDisposition,
    pub requested_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub executor: String,
    #[serde(default)]
    pub servers: Vec<McpServerRefreshSnapshot>,
    pub error: Option<McpRefreshError>,
}
