use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqlitePool};

/// The host's Settings → CHNG row. The `encrypted_*` columns are opaque
/// envelopes; callers decrypt them through the services layer and must never
/// log them.
#[derive(Clone, Default, FromRow)]
pub struct ChngSettingsRow {
    pub jira_base_url: String,
    pub jira_email: String,
    pub jql: String,
    pub encrypted_jira_token: Option<String>,
    pub hashmal_mcp_url: String,
    pub encrypted_hashmal_token: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for ChngSettingsRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChngSettingsRow")
            .field("jira_base_url", &self.jira_base_url)
            .field("jira_email", &self.jira_email)
            .field("jql", &self.jql)
            .field("has_jira_token", &self.encrypted_jira_token.is_some())
            .field("hashmal_mcp_url", &self.hashmal_mcp_url)
            .field("has_hashmal_token", &self.encrypted_hashmal_token.is_some())
            .finish_non_exhaustive()
    }
}

impl ChngSettingsRow {
    /// The stored row, or `None` before the first save.
    pub async fn get(pool: &SqlitePool) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            "SELECT jira_base_url, jira_email, jql, encrypted_jira_token, hashmal_mcp_url, \
             encrypted_hashmal_token, updated_at FROM chng_settings WHERE id = 1",
        )
        .fetch_optional(pool)
        .await
    }

    /// Writes `next` only if the destinations still match `expected` (the row
    /// the caller validated against, `None` meaning "no row yet"). A stored
    /// token must never be sent to a URL other than the one it was saved with,
    /// so that check is atomic with the write. Returns `false` when the row
    /// changed underneath the caller.
    pub async fn save_if_unchanged(
        pool: &SqlitePool,
        expected: Option<&Self>,
        next: &Self,
    ) -> Result<bool, sqlx::Error> {
        let result = match expected {
            None => {
                sqlx::query(
                    "INSERT INTO chng_settings (id, jira_base_url, jira_email, jql, \
                     encrypted_jira_token, hashmal_mcp_url, encrypted_hashmal_token) \
                     VALUES (1, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO NOTHING",
                )
                .bind(&next.jira_base_url)
                .bind(&next.jira_email)
                .bind(&next.jql)
                .bind(&next.encrypted_jira_token)
                .bind(&next.hashmal_mcp_url)
                .bind(&next.encrypted_hashmal_token)
                .execute(pool)
                .await?
            }
            Some(expected) => {
                sqlx::query(
                    "UPDATE chng_settings SET jira_base_url = ?, jira_email = ?, jql = ?, \
                     encrypted_jira_token = ?, hashmal_mcp_url = ?, encrypted_hashmal_token = ?, \
                     updated_at = datetime('now', 'subsec') \
                     WHERE id = 1 AND jira_base_url = ? AND hashmal_mcp_url = ? \
                     AND encrypted_jira_token IS ? AND encrypted_hashmal_token IS ?",
                )
                .bind(&next.jira_base_url)
                .bind(&next.jira_email)
                .bind(&next.jql)
                .bind(&next.encrypted_jira_token)
                .bind(&next.hashmal_mcp_url)
                .bind(&next.encrypted_hashmal_token)
                .bind(&expected.jira_base_url)
                .bind(&expected.hashmal_mcp_url)
                .bind(&expected.encrypted_jira_token)
                .bind(&expected.encrypted_hashmal_token)
                .execute(pool)
                .await?
            }
        };
        Ok(result.rows_affected() == 1)
    }
}
