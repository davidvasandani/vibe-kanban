//! Repository for `organization_slack_configs`.
//!
//! Uses runtime-checked queries (`sqlx::query_as`) rather than the `query!`
//! macros: regenerating SQLx offline metadata requires a running Postgres,
//! which the implementation environment may not have. Same recorded
//! deviation as the Jira sync repository fallback.

use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use crate::slack::types::SlackConfig;

#[derive(Debug, Error)]
pub enum SlackConfigDbError {
    #[error("another organization is already connected to this Slack workspace")]
    TeamAlreadyConnected,
    #[error("database error: {0}")]
    Database(sqlx::Error),
}

impl From<sqlx::Error> for SlackConfigDbError {
    fn from(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db_err) = &err
            && db_err.constraint() == Some("organization_slack_configs_slack_team_id_key")
        {
            return SlackConfigDbError::TeamAlreadyConnected;
        }
        SlackConfigDbError::Database(err)
    }
}

const SELECT_COLUMNS: &str = r#"
    id, organization_id, encrypted_bot_token, encrypted_signing_secret,
    slack_team_id, slack_team_name, enabled, created_by_user_id,
    encrypted_anthropic_api_key, ai_summarization_enabled,
    created_at, updated_at
"#;

/// Arguments for creating/updating a config row. Credentials are already
/// encrypted by the caller; `None` keeps the stored value on update (the
/// route requires both on first save). `slack_team_id`/`slack_team_name`
/// are `Some` exactly when a new bot token was validated via `auth.test`.
#[derive(Debug)]
pub struct UpsertSlackConfigArgs {
    pub organization_id: Uuid,
    pub encrypted_bot_token: Option<String>,
    pub encrypted_signing_secret: Option<String>,
    pub slack_team_id: Option<String>,
    pub slack_team_name: Option<String>,
    pub enabled: bool,
    pub created_by_user_id: Uuid,
    /// AES-256-GCM ciphertext of the Anthropic API key. `None` keeps the
    /// stored value (write-only, same as the bot token); `Some("")` is never
    /// passed — the route filters empty input to `None`.
    pub encrypted_anthropic_api_key: Option<String>,
    pub ai_summarization_enabled: bool,
}

pub struct SlackConfigRepository;

impl SlackConfigRepository {
    pub async fn find_by_organization(
        pool: &PgPool,
        organization_id: Uuid,
    ) -> Result<Option<SlackConfig>, SlackConfigDbError> {
        let query = format!(
            "SELECT {SELECT_COLUMNS} FROM organization_slack_configs WHERE organization_id = $1"
        );
        let record = sqlx::query_as::<_, SlackConfig>(&query)
            .bind(organization_id)
            .fetch_optional(pool)
            .await?;
        Ok(record)
    }

    /// Lookup for inbound interaction payloads. Deliberately NOT filtered on
    /// `enabled`: the row carries the signing secret needed to verify the
    /// request, and the disabled branch must still answer the user (FR-7).
    pub async fn find_by_team(
        pool: &PgPool,
        slack_team_id: &str,
    ) -> Result<Option<SlackConfig>, SlackConfigDbError> {
        let query = format!(
            "SELECT {SELECT_COLUMNS} FROM organization_slack_configs WHERE slack_team_id = $1"
        );
        let record = sqlx::query_as::<_, SlackConfig>(&query)
            .bind(slack_team_id)
            .fetch_optional(pool)
            .await?;
        Ok(record)
    }

    /// Create or update the org's config. On conflict, NULL credential/team
    /// args keep the stored values (write-only semantics). The row's
    /// `created_by_user_id` is always set to the saving admin — they become
    /// the attribution target for issues created via Slack (FR-15).
    pub async fn upsert(
        pool: &PgPool,
        args: UpsertSlackConfigArgs,
    ) -> Result<SlackConfig, SlackConfigDbError> {
        let query = format!(
            r#"
            INSERT INTO organization_slack_configs (
                organization_id, encrypted_bot_token, encrypted_signing_secret,
                slack_team_id, slack_team_name, enabled, created_by_user_id,
                encrypted_anthropic_api_key, ai_summarization_enabled
            )
            VALUES ($1, COALESCE($2, ''), COALESCE($3, ''), COALESCE($4, ''), COALESCE($5, ''), $6, $7, $8, $9)
            ON CONFLICT (organization_id) DO UPDATE SET
                encrypted_bot_token =
                    COALESCE($2, organization_slack_configs.encrypted_bot_token),
                encrypted_signing_secret =
                    COALESCE($3, organization_slack_configs.encrypted_signing_secret),
                slack_team_id = COALESCE($4, organization_slack_configs.slack_team_id),
                slack_team_name = COALESCE($5, organization_slack_configs.slack_team_name),
                enabled = $6,
                created_by_user_id = $7,
                encrypted_anthropic_api_key =
                    COALESCE($8, organization_slack_configs.encrypted_anthropic_api_key),
                ai_summarization_enabled = $9
            RETURNING {SELECT_COLUMNS}
            "#
        );
        let record = sqlx::query_as::<_, SlackConfig>(&query)
            .bind(args.organization_id)
            .bind(args.encrypted_bot_token)
            .bind(args.encrypted_signing_secret)
            .bind(args.slack_team_id)
            .bind(args.slack_team_name)
            .bind(args.enabled)
            .bind(args.created_by_user_id)
            .bind(args.encrypted_anthropic_api_key)
            .bind(args.ai_summarization_enabled)
            .fetch_one(pool)
            .await?;
        Ok(record)
    }

    /// Disconnect. Issues created from Slack are untouched (FR-12).
    /// Returns whether a config existed.
    pub async fn delete(pool: &PgPool, organization_id: Uuid) -> Result<bool, SlackConfigDbError> {
        let deleted =
            sqlx::query("DELETE FROM organization_slack_configs WHERE organization_id = $1")
                .bind(organization_id)
                .execute(pool)
                .await?;
        Ok(deleted.rows_affected() > 0)
    }

    /// Sort order for an issue appended to a project's board, matching the
    /// Jira sync convention (max + 1).
    pub async fn next_sort_order(
        pool: &PgPool,
        project_id: Uuid,
    ) -> Result<f64, SlackConfigDbError> {
        let max: Option<f64> =
            sqlx::query_scalar("SELECT MAX(sort_order) FROM issues WHERE project_id = $1")
                .bind(project_id)
                .fetch_one(pool)
                .await?;
        Ok(max.unwrap_or(0.0) + 1.0)
    }

    /// Idempotency lookup: the issue already created for a given Slack modal
    /// instance, if any. A `view_submission` replayed within the signature
    /// window is byte-identical, so it carries the same `view.id`; finding a
    /// hit means "already handled" (same pattern as the Jira sync's
    /// metadata-based orphan re-link).
    pub async fn find_issue_id_by_slack_view(
        pool: &PgPool,
        project_id: Uuid,
        view_id: &str,
    ) -> Result<Option<Uuid>, SlackConfigDbError> {
        let id: Option<Uuid> = sqlx::query_scalar(
            r#"
            SELECT id FROM issues
            WHERE project_id = $1 AND extension_metadata #>> '{slack,view_id}' = $2
            LIMIT 1
            "#,
        )
        .bind(project_id)
        .bind(view_id)
        .fetch_optional(pool)
        .await?;
        Ok(id)
    }
}
