use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Organization-scoped environment variable. Values are never returned in
/// list/listing responses to avoid exposing secrets; clients overwrite by
/// PATCHing a new value.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, TS)]
pub struct OrganizationEnvVar {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListOrganizationEnvVarsResponse {
    pub env_vars: Vec<OrganizationEnvVar>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateOrganizationEnvVarRequest {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateOrganizationEnvVarResponse {
    pub env_var: OrganizationEnvVar,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateOrganizationEnvVarRequest {
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateOrganizationEnvVarResponse {
    pub env_var: OrganizationEnvVar,
}

/// A resolved (decrypted) organization env var. Unlike the listing types above,
/// this carries the plaintext `value` and is only returned to callers with
/// access to the owning organization's project, for injection into agent
/// processes started against that project.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ResolvedEnvVar {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ResolvedOrganizationEnvVarsResponse {
    pub env_vars: Vec<ResolvedEnvVar>,
}
