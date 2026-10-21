use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Workspace metadata pushed from local clients
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, TS)]
pub struct Workspace {
    pub id: Uuid,
    pub project_id: Uuid,
    pub owner_user_id: Uuid,
    pub issue_id: Option<Uuid>,
    pub local_workspace_id: Option<Uuid>,
    pub name: Option<String>,
    pub archived: bool,
    pub files_changed: Option<i32>,
    pub lines_added: Option<i32>,
    pub lines_removed: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Workspaces linked to one issue, for routing issue comments to sessions.
/// `workspaces` holds only the active (non-archived) links; archived links are
/// counted so callers can explain why nothing is eligible.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListIssueWorkspacesResponse {
    pub workspaces: Vec<Workspace>,
    pub archived_count: i64,
}
