use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// How the stored credential is presented to Jira.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum JiraAuthMode {
    /// Jira Cloud: email + API token via HTTP Basic auth.
    CloudBasic,
    /// Jira Server / Data Center: personal access token via Bearer auth.
    ServerPat,
}

impl JiraAuthMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            JiraAuthMode::CloudBasic => "cloud_basic",
            JiraAuthMode::ServerPat => "server_pat",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "cloud_basic" => Some(JiraAuthMode::CloudBasic),
            "server_pat" => Some(JiraAuthMode::ServerPat),
            _ => None,
        }
    }
}

/// User-editable status mapping stored as JSONB on the config row.
///
/// `jira_to_vk` overrides the per-status-category default when mapping
/// inbound Jira statuses onto VK board columns; `vk_to_jira` names the Jira
/// status to transition to when a VK column change is pushed outbound. Both
/// are keyed/valued by status *names*.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct JiraStatusMapping {
    #[serde(default)]
    pub jira_to_vk: BTreeMap<String, String>,
    #[serde(default)]
    pub vk_to_jira: BTreeMap<String, String>,
}

/// Full config row. Internal only — carries the encrypted credential and is
/// never serialized to clients (see [`JiraSyncConfigResponse`]).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct JiraSyncConfig {
    pub id: Uuid,
    pub project_id: Uuid,
    pub jira_base_url: String,
    pub auth_mode: String,
    pub jira_email: Option<String>,
    pub encrypted_credential: String,
    pub jql: String,
    pub enabled: bool,
    pub created_by_user_id: Option<Uuid>,
    pub sync_interval_seconds: i32,
    pub status_mapping: serde_json::Value,
    pub sync_requested_at: Option<DateTime<Utc>>,
    pub last_sync_started_at: Option<DateTime<Utc>>,
    pub last_sync_completed_at: Option<DateTime<Utc>>,
    pub last_sync_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl JiraSyncConfig {
    pub fn parsed_status_mapping(&self) -> JiraStatusMapping {
        serde_json::from_value(self.status_mapping.clone()).unwrap_or_default()
    }
}

/// One row per Jira issue ever synced into the project. Streamed to boards
/// via the `PROJECT_JIRA_LINKS_SHAPE` Electric shape (contains no secrets).
#[derive(Debug, Clone, Serialize, Deserialize, TS, sqlx::FromRow)]
pub struct JiraIssueLink {
    pub id: Uuid,
    pub config_id: Uuid,
    pub project_id: Uuid,
    pub issue_id: Uuid,
    pub jira_issue_id: String,
    pub jira_issue_key: String,
    pub jira_browse_url: String,
    /// `active` | `dormant` (left the JQL scope) | `deleted_remote`.
    pub link_state: String,
    pub last_synced_title: Option<String>,
    pub last_synced_description: Option<String>,
    pub last_synced_status_id: Option<Uuid>,
    pub last_synced_jira_status: Option<String>,
    pub last_synced_jira_updated_at: Option<DateTime<Utc>>,
    pub last_synced_vk_updated_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub const LINK_STATE_ACTIVE: &str = "active";
pub const LINK_STATE_DORMANT: &str = "dormant";
pub const LINK_STATE_DELETED_REMOTE: &str = "deleted_remote";

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct JiraLinkCounts {
    pub active: i64,
    pub dormant: i64,
    pub deleted_remote: i64,
    pub errored: i64,
}

/// Client-facing view of the config: credential replaced by `has_credential`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct JiraSyncConfigResponse {
    pub project_id: Uuid,
    pub jira_base_url: String,
    pub auth_mode: JiraAuthMode,
    pub jira_email: Option<String>,
    pub has_credential: bool,
    pub jql: String,
    pub enabled: bool,
    pub sync_interval_seconds: i32,
    pub status_mapping: JiraStatusMapping,
    pub sync_requested_at: Option<DateTime<Utc>>,
    pub last_sync_started_at: Option<DateTime<Utc>>,
    pub last_sync_completed_at: Option<DateTime<Utc>>,
    pub last_sync_error: Option<String>,
    pub link_counts: JiraLinkCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpsertJiraSyncConfigRequest {
    pub jira_base_url: String,
    pub auth_mode: JiraAuthMode,
    pub jira_email: Option<String>,
    /// Write-only. `None` on update keeps the stored credential; required on
    /// first create.
    pub credential: Option<String>,
    pub jql: String,
    pub enabled: bool,
    pub sync_interval_seconds: i32,
    pub status_mapping: JiraStatusMapping,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct JiraTestConnectionRequest {
    pub jira_base_url: String,
    pub auth_mode: JiraAuthMode,
    pub jira_email: Option<String>,
    /// Falls back to the stored credential when `None`.
    pub credential: Option<String>,
    pub jql: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct JiraTestConnectionResponse {
    pub ok: bool,
    /// `None` when the deployment can't provide a count (e.g. Cloud without
    /// the approximate-count endpoint).
    pub match_count: Option<i64>,
    /// Distinct Jira status names seen on the first page; seeds the mapping UI.
    pub jira_statuses: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct JiraSyncNowResponse {
    pub requested_at: DateTime<Utc>,
}
