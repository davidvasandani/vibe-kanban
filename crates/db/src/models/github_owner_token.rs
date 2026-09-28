use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

/// A stored per-owner GitHub token. `encrypted_value` is an opaque envelope;
/// callers decrypt it through the services layer and must never log it.
#[derive(Clone, FromRow)]
pub struct GitHubOwnerTokenRow {
    pub id: Uuid,
    pub owner: String,
    pub encrypted_value: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl std::fmt::Debug for GitHubOwnerTokenRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitHubOwnerTokenRow")
            .field("id", &self.id)
            .field("owner", &self.owner)
            .finish_non_exhaustive()
    }
}

impl GitHubOwnerTokenRow {
    const SELECT: &'static str =
        "SELECT id, owner, encrypted_value, created_at, updated_at FROM github_owner_tokens";

    pub async fn list(pool: &SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(&format!("{} ORDER BY owner COLLATE NOCASE", Self::SELECT))
            .fetch_all(pool)
            .await
    }

    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(&format!("{} WHERE id = ?", Self::SELECT))
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    /// Inserts a row with a caller-chosen id, so the id can be bound into the
    /// envelope before the row exists. A case-insensitive duplicate owner
    /// fails with the database's unique-constraint error.
    pub async fn create(
        pool: &SqlitePool,
        id: Uuid,
        owner: &str,
        encrypted_value: &str,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query(
            "INSERT INTO github_owner_tokens (id, owner, encrypted_value) VALUES (?, ?, ?)",
        )
        .bind(id)
        .bind(owner)
        .bind(encrypted_value)
        .execute(pool)
        .await?;
        Self::find_by_id(pool, id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn update_value(
        pool: &SqlitePool,
        id: Uuid,
        encrypted_value: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE github_owner_tokens SET encrypted_value = ?, updated_at = datetime('now', 'subsec') WHERE id = ?",
        )
        .bind(encrypted_value)
        .bind(id)
        .execute(pool)
        .await?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        Self::find_by_id(pool, id).await
    }

    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM github_owner_tokens WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() == 1)
    }
}
