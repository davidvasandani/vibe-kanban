use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres};
use thiserror::Error;
use uuid::Uuid;

use crate::jira::types::{JiraIssueLink, JiraLinkCounts, JiraSyncConfig};

#[derive(Debug, Error)]
pub enum JiraSyncDbError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub struct JiraSyncRepository;

/// Arguments for creating/updating a config row (the credential is already
/// encrypted by the caller; `None` keeps the stored one on update).
#[derive(Debug)]
pub struct UpsertJiraSyncConfigArgs {
    pub project_id: Uuid,
    pub jira_base_url: String,
    pub auth_mode: String,
    pub jira_email: Option<String>,
    pub encrypted_credential: Option<String>,
    pub jql: String,
    pub enabled: bool,
    pub sync_interval_seconds: i32,
    pub status_mapping: serde_json::Value,
    pub created_by_user_id: Uuid,
}

/// Snapshot values written after a field-set converges (the 3-way merge base).
#[derive(Debug, Clone)]
pub struct LinkSnapshot {
    pub title: String,
    pub description: Option<String>,
    pub status_id: Uuid,
    pub jira_status: String,
    pub jira_updated_at: Option<DateTime<Utc>>,
    pub vk_updated_at: DateTime<Utc>,
}

impl JiraSyncRepository {
    pub async fn find_config_by_project(
        pool: &PgPool,
        project_id: Uuid,
    ) -> Result<Option<JiraSyncConfig>, JiraSyncDbError> {
        let record = sqlx::query_as!(
            JiraSyncConfig,
            r#"
            SELECT
                id                      AS "id!: Uuid",
                project_id              AS "project_id!: Uuid",
                jira_base_url           AS "jira_base_url!",
                auth_mode               AS "auth_mode!",
                jira_email              AS "jira_email?",
                encrypted_credential    AS "encrypted_credential!",
                jql                     AS "jql!",
                enabled                 AS "enabled!",
                created_by_user_id      AS "created_by_user_id?: Uuid",
                sync_interval_seconds   AS "sync_interval_seconds!",
                status_mapping          AS "status_mapping!: serde_json::Value",
                sync_requested_at       AS "sync_requested_at?: DateTime<Utc>",
                last_sync_started_at    AS "last_sync_started_at?: DateTime<Utc>",
                last_sync_completed_at  AS "last_sync_completed_at?: DateTime<Utc>",
                last_sync_error         AS "last_sync_error?",
                created_at              AS "created_at!: DateTime<Utc>",
                updated_at              AS "updated_at!: DateTime<Utc>"
            FROM project_jira_configs
            WHERE project_id = $1
            "#,
            project_id
        )
        .fetch_optional(pool)
        .await?;
        Ok(record)
    }

    pub async fn upsert_config(
        pool: &PgPool,
        args: UpsertJiraSyncConfigArgs,
    ) -> Result<JiraSyncConfig, JiraSyncDbError> {
        let record = sqlx::query_as!(
            JiraSyncConfig,
            r#"
            INSERT INTO project_jira_configs (
                project_id, jira_base_url, auth_mode, jira_email,
                encrypted_credential, jql, enabled, sync_interval_seconds,
                status_mapping, created_by_user_id
            )
            VALUES ($1, $2, $3, $4, COALESCE($5, ''), $6, $7, $8, $9, $10)
            ON CONFLICT (project_id) DO UPDATE SET
                jira_base_url = EXCLUDED.jira_base_url,
                auth_mode = EXCLUDED.auth_mode,
                jira_email = EXCLUDED.jira_email,
                encrypted_credential = COALESCE($5, project_jira_configs.encrypted_credential),
                jql = EXCLUDED.jql,
                enabled = EXCLUDED.enabled,
                sync_interval_seconds = EXCLUDED.sync_interval_seconds,
                status_mapping = EXCLUDED.status_mapping
            RETURNING
                id                      AS "id!: Uuid",
                project_id              AS "project_id!: Uuid",
                jira_base_url           AS "jira_base_url!",
                auth_mode               AS "auth_mode!",
                jira_email              AS "jira_email?",
                encrypted_credential    AS "encrypted_credential!",
                jql                     AS "jql!",
                enabled                 AS "enabled!",
                created_by_user_id      AS "created_by_user_id?: Uuid",
                sync_interval_seconds   AS "sync_interval_seconds!",
                status_mapping          AS "status_mapping!: serde_json::Value",
                sync_requested_at       AS "sync_requested_at?: DateTime<Utc>",
                last_sync_started_at    AS "last_sync_started_at?: DateTime<Utc>",
                last_sync_completed_at  AS "last_sync_completed_at?: DateTime<Utc>",
                last_sync_error         AS "last_sync_error?",
                created_at              AS "created_at!: DateTime<Utc>",
                updated_at              AS "updated_at!: DateTime<Utc>"
            "#,
            args.project_id,
            args.jira_base_url,
            args.auth_mode,
            args.jira_email,
            args.encrypted_credential,
            args.jql,
            args.enabled,
            args.sync_interval_seconds,
            args.status_mapping,
            args.created_by_user_id
        )
        .fetch_one(pool)
        .await?;
        Ok(record)
    }

    /// Persist a changed status mapping (e.g. after seeding `vk_to_jira`).
    pub async fn update_status_mapping(
        pool: &PgPool,
        config_id: Uuid,
        status_mapping: serde_json::Value,
    ) -> Result<(), JiraSyncDbError> {
        sqlx::query!(
            "UPDATE project_jira_configs SET status_mapping = $2 WHERE id = $1",
            config_id,
            status_mapping
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Delete the config and all links. VK issues are untouched (FR-4).
    /// Returns whether a config existed.
    pub async fn delete_config(pool: &PgPool, project_id: Uuid) -> Result<bool, JiraSyncDbError> {
        let mut tx = pool.begin().await?;
        sqlx::query!(
            "DELETE FROM jira_issue_links WHERE project_id = $1",
            project_id
        )
        .execute(&mut *tx)
        .await?;
        let deleted = sqlx::query!(
            "DELETE FROM project_jira_configs WHERE project_id = $1",
            project_id
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(deleted.rows_affected() > 0)
    }

    /// Level-triggered "sync now": the reconciler runs a pass when
    /// `sync_requested_at` is newer than the last started pass.
    pub async fn request_sync_now(
        pool: &PgPool,
        project_id: Uuid,
    ) -> Result<Option<DateTime<Utc>>, JiraSyncDbError> {
        let record = sqlx::query_scalar!(
            r#"
            UPDATE project_jira_configs
            SET sync_requested_at = NOW()
            WHERE project_id = $1
            RETURNING sync_requested_at AS "sync_requested_at!: DateTime<Utc>"
            "#,
            project_id
        )
        .fetch_optional(pool)
        .await?;
        Ok(record)
    }

    /// Configs due for a pass: enabled AND (never synced, interval elapsed,
    /// or an unserviced sync-now request). A pass that crashed before
    /// completing keeps `last_sync_completed_at` stale, so it is retried on
    /// the next tick (level-triggered, constitution VI).
    pub async fn list_due_configs(pool: &PgPool) -> Result<Vec<JiraSyncConfig>, JiraSyncDbError> {
        let records = sqlx::query_as!(
            JiraSyncConfig,
            r#"
            SELECT
                id                      AS "id!: Uuid",
                project_id              AS "project_id!: Uuid",
                jira_base_url           AS "jira_base_url!",
                auth_mode               AS "auth_mode!",
                jira_email              AS "jira_email?",
                encrypted_credential    AS "encrypted_credential!",
                jql                     AS "jql!",
                enabled                 AS "enabled!",
                created_by_user_id      AS "created_by_user_id?: Uuid",
                sync_interval_seconds   AS "sync_interval_seconds!",
                status_mapping          AS "status_mapping!: serde_json::Value",
                sync_requested_at       AS "sync_requested_at?: DateTime<Utc>",
                last_sync_started_at    AS "last_sync_started_at?: DateTime<Utc>",
                last_sync_completed_at  AS "last_sync_completed_at?: DateTime<Utc>",
                last_sync_error         AS "last_sync_error?",
                created_at              AS "created_at!: DateTime<Utc>",
                updated_at              AS "updated_at!: DateTime<Utc>"
            FROM project_jira_configs
            WHERE enabled
              AND (
                last_sync_completed_at IS NULL
                OR last_sync_completed_at
                   <= NOW() - make_interval(secs => sync_interval_seconds::double precision)
                OR sync_requested_at > COALESCE(last_sync_started_at, '-infinity'::timestamptz)
              )
            ORDER BY last_sync_completed_at ASC NULLS FIRST
            "#
        )
        .fetch_all(pool)
        .await?;
        Ok(records)
    }

    pub async fn mark_sync_started(pool: &PgPool, config_id: Uuid) -> Result<(), JiraSyncDbError> {
        sqlx::query!(
            "UPDATE project_jira_configs SET last_sync_started_at = NOW() WHERE id = $1",
            config_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn mark_sync_completed(
        pool: &PgPool,
        config_id: Uuid,
        error: Option<String>,
    ) -> Result<(), JiraSyncDbError> {
        sqlx::query!(
            r#"
            UPDATE project_jira_configs
            SET last_sync_completed_at = NOW(), last_sync_error = $2
            WHERE id = $1
            "#,
            config_id,
            error
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Links
    // ------------------------------------------------------------------

    pub async fn list_links_by_project(
        pool: &PgPool,
        project_id: Uuid,
    ) -> Result<Vec<JiraIssueLink>, JiraSyncDbError> {
        let records = sqlx::query_as!(
            JiraIssueLink,
            r#"
            SELECT
                id                          AS "id!: Uuid",
                project_id                  AS "project_id!: Uuid",
                issue_id                    AS "issue_id!: Uuid",
                jira_issue_id               AS "jira_issue_id!",
                jira_issue_key              AS "jira_issue_key!",
                jira_browse_url             AS "jira_browse_url!",
                link_state                  AS "link_state!",
                last_synced_title           AS "last_synced_title?",
                last_synced_description     AS "last_synced_description?",
                last_synced_status_id       AS "last_synced_status_id?: Uuid",
                last_synced_jira_status     AS "last_synced_jira_status?",
                last_synced_jira_updated_at AS "last_synced_jira_updated_at?: DateTime<Utc>",
                last_synced_vk_updated_at   AS "last_synced_vk_updated_at?: DateTime<Utc>",
                last_error                  AS "last_error?",
                created_at                  AS "created_at!: DateTime<Utc>",
                updated_at                  AS "updated_at!: DateTime<Utc>"
            FROM jira_issue_links
            WHERE project_id = $1
            ORDER BY created_at ASC
            "#,
            project_id
        )
        .fetch_all(pool)
        .await?;
        Ok(records)
    }

    /// Insert the link row for a VK issue the sync just created.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_link<'e, E>(
        executor: E,
        project_id: Uuid,
        issue_id: Uuid,
        jira_issue_id: &str,
        jira_issue_key: &str,
        jira_browse_url: &str,
        snapshot: &LinkSnapshot,
    ) -> Result<JiraIssueLink, JiraSyncDbError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let record = sqlx::query_as!(
            JiraIssueLink,
            r#"
            INSERT INTO jira_issue_links (
                project_id, issue_id, jira_issue_id, jira_issue_key,
                jira_browse_url, link_state,
                last_synced_title, last_synced_description,
                last_synced_status_id, last_synced_jira_status,
                last_synced_jira_updated_at, last_synced_vk_updated_at
            )
            VALUES ($1, $2, $3, $4, $5, 'active', $6, $7, $8, $9, $10, $11)
            RETURNING
                id                          AS "id!: Uuid",
                project_id                  AS "project_id!: Uuid",
                issue_id                    AS "issue_id!: Uuid",
                jira_issue_id               AS "jira_issue_id!",
                jira_issue_key              AS "jira_issue_key!",
                jira_browse_url             AS "jira_browse_url!",
                link_state                  AS "link_state!",
                last_synced_title           AS "last_synced_title?",
                last_synced_description     AS "last_synced_description?",
                last_synced_status_id       AS "last_synced_status_id?: Uuid",
                last_synced_jira_status     AS "last_synced_jira_status?",
                last_synced_jira_updated_at AS "last_synced_jira_updated_at?: DateTime<Utc>",
                last_synced_vk_updated_at   AS "last_synced_vk_updated_at?: DateTime<Utc>",
                last_error                  AS "last_error?",
                created_at                  AS "created_at!: DateTime<Utc>",
                updated_at                  AS "updated_at!: DateTime<Utc>"
            "#,
            project_id,
            issue_id,
            jira_issue_id,
            jira_issue_key,
            jira_browse_url,
            snapshot.title.as_str(),
            snapshot.description.as_deref(),
            snapshot.status_id,
            snapshot.jira_status.as_str(),
            snapshot.jira_updated_at,
            snapshot.vk_updated_at
        )
        .fetch_one(executor)
        .await?;
        Ok(record)
    }

    /// Refresh the snapshot after a field-set converges, reactivating the
    /// link and clearing any recorded error. Also refreshes key/URL (Jira
    /// keys can change when issues move projects).
    pub async fn update_link_snapshot<'e, E>(
        executor: E,
        link_id: Uuid,
        jira_issue_key: &str,
        jira_browse_url: &str,
        snapshot: &LinkSnapshot,
    ) -> Result<(), JiraSyncDbError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query!(
            r#"
            UPDATE jira_issue_links
            SET jira_issue_key = $2,
                jira_browse_url = $3,
                link_state = 'active',
                last_synced_title = $4,
                last_synced_description = $5,
                last_synced_status_id = $6,
                last_synced_jira_status = $7,
                last_synced_jira_updated_at = $8,
                last_synced_vk_updated_at = $9,
                last_error = NULL
            WHERE id = $1
            "#,
            link_id,
            jira_issue_key,
            jira_browse_url,
            snapshot.title.as_str(),
            snapshot.description.as_deref(),
            snapshot.status_id,
            snapshot.jira_status.as_str(),
            snapshot.jira_updated_at,
            snapshot.vk_updated_at
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn set_link_state(
        pool: &PgPool,
        link_id: Uuid,
        link_state: &str,
    ) -> Result<(), JiraSyncDbError> {
        sqlx::query!(
            "UPDATE jira_issue_links SET link_state = $2 WHERE id = $1",
            link_id,
            link_state
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn set_link_error(
        pool: &PgPool,
        link_id: Uuid,
        error: Option<String>,
    ) -> Result<(), JiraSyncDbError> {
        sqlx::query!(
            "UPDATE jira_issue_links SET last_error = $2 WHERE id = $1",
            link_id,
            error
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Recover from a crash between issue creation and link creation: sync-
    /// created issues carry their Jira identity in `extension_metadata`, so a
    /// matching orphan can be re-linked instead of duplicated (FR-5).
    pub async fn find_issue_id_by_jira_metadata(
        pool: &PgPool,
        project_id: Uuid,
        jira_issue_id: &str,
    ) -> Result<Option<Uuid>, JiraSyncDbError> {
        let record = sqlx::query_scalar!(
            r#"
            SELECT id AS "id!: Uuid"
            FROM issues
            WHERE project_id = $1
              AND extension_metadata #>> '{jira,issue_id}' = $2
            LIMIT 1
            "#,
            project_id,
            jira_issue_id
        )
        .fetch_optional(pool)
        .await?;
        Ok(record)
    }

    /// Sort order that appends a new issue at the end of the project board.
    pub async fn next_sort_order(
        pool: &PgPool,
        project_id: Uuid,
    ) -> Result<f64, JiraSyncDbError> {
        let max = sqlx::query_scalar!(
            r#"SELECT MAX(sort_order) AS "max?: f64" FROM issues WHERE project_id = $1"#,
            project_id
        )
        .fetch_one(pool)
        .await?;
        Ok(max.unwrap_or(0.0) + 1.0)
    }

    pub async fn link_counts(
        pool: &PgPool,
        project_id: Uuid,
    ) -> Result<JiraLinkCounts, JiraSyncDbError> {
        let record = sqlx::query!(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE link_state = 'active')         AS "active!",
                COUNT(*) FILTER (WHERE link_state = 'dormant')        AS "dormant!",
                COUNT(*) FILTER (WHERE link_state = 'deleted_remote') AS "deleted_remote!",
                COUNT(*) FILTER (WHERE last_error IS NOT NULL)        AS "errored!"
            FROM jira_issue_links
            WHERE project_id = $1
            "#,
            project_id
        )
        .fetch_one(pool)
        .await?;
        Ok(JiraLinkCounts {
            active: record.active,
            dormant: record.dormant,
            deleted_remote: record.deleted_remote,
            errored: record.errored,
        })
    }
}
