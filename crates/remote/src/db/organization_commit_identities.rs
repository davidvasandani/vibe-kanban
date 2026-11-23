//! Repository for `organization_commit_identities`.
//!
//! Uses runtime-checked queries (`sqlx::query_as`) rather than the `query!`
//! macros: regenerating SQLx offline metadata requires a running Postgres,
//! which the implementation environment may not have. Same recorded
//! deviation as the Slack config repository.

use api_types::{NormalizedCommitIdentity, OrganizationCommitIdentity};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

const SELECT_COLUMNS: &str = "organization_id, name, email, allowed_emails, updated_at";

#[derive(sqlx::FromRow)]
struct CommitIdentityRow {
    organization_id: Uuid,
    name: String,
    email: String,
    allowed_emails: Vec<String>,
    updated_at: DateTime<Utc>,
}

impl From<CommitIdentityRow> for OrganizationCommitIdentity {
    fn from(row: CommitIdentityRow) -> Self {
        Self {
            organization_id: row.organization_id,
            name: row.name,
            email: row.email,
            allowed_emails: row.allowed_emails,
            updated_at: row.updated_at,
        }
    }
}

pub struct OrganizationCommitIdentityRepository;

impl OrganizationCommitIdentityRepository {
    pub async fn find_by_organization(
        pool: &PgPool,
        organization_id: Uuid,
    ) -> Result<Option<OrganizationCommitIdentity>, sqlx::Error> {
        let query = format!(
            "SELECT {SELECT_COLUMNS} FROM organization_commit_identities WHERE organization_id = $1"
        );
        let row = sqlx::query_as::<_, CommitIdentityRow>(&query)
            .bind(organization_id)
            .fetch_optional(pool)
            .await?;
        Ok(row.map(Into::into))
    }

    pub async fn upsert(
        pool: &PgPool,
        organization_id: Uuid,
        identity: &NormalizedCommitIdentity,
        updated_by_user_id: Uuid,
    ) -> Result<OrganizationCommitIdentity, sqlx::Error> {
        let query = format!(
            r#"
            INSERT INTO organization_commit_identities (
                organization_id, name, email, allowed_emails, updated_by_user_id
            )
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (organization_id) DO UPDATE SET
                name = EXCLUDED.name,
                email = EXCLUDED.email,
                allowed_emails = EXCLUDED.allowed_emails,
                updated_by_user_id = EXCLUDED.updated_by_user_id
            RETURNING {SELECT_COLUMNS}
            "#
        );
        let row = sqlx::query_as::<_, CommitIdentityRow>(&query)
            .bind(organization_id)
            .bind(&identity.name)
            .bind(&identity.email)
            .bind(&identity.allowed_emails)
            .bind(updated_by_user_id)
            .fetch_one(pool)
            .await?;
        Ok(row.into())
    }

    /// Returns whether a row was deleted.
    pub async fn delete(pool: &PgPool, organization_id: Uuid) -> Result<bool, sqlx::Error> {
        let result =
            sqlx::query("DELETE FROM organization_commit_identities WHERE organization_id = $1")
                .bind(organization_id)
                .execute(pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }
}
