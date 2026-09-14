use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
pub struct PreviewLease {
    pub id: Uuid,
    #[serde(skip_serializing)]
    #[ts(skip)]
    pub token_digest: String,
    pub workspace_id: Uuid,
    pub execution_process_id: Uuid,
    pub target_port: i64,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl PreviewLease {
    const SELECT: &'static str = "SELECT id, token_digest, workspace_id, execution_process_id, target_port, created_at, expires_at, revoked_at FROM preview_leases";

    pub async fn create(
        pool: &SqlitePool,
        workspace_id: Uuid,
        execution_process_id: Uuid,
        target_port: u16,
        token_digest: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<Self, sqlx::Error> {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO preview_leases (id, token_digest, workspace_id, execution_process_id, target_port, expires_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(id)
            .bind(token_digest)
            .bind(workspace_id)
            .bind(execution_process_id)
            .bind(i64::from(target_port))
            .bind(expires_at)
            .execute(pool)
            .await?;
        Self::find_by_id(pool, id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(&format!("{} WHERE id = ?", Self::SELECT))
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    pub async fn find_by_digest(
        pool: &SqlitePool,
        digest: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(&format!("{} WHERE token_digest = ?", Self::SELECT))
            .bind(digest)
            .fetch_optional(pool)
            .await
    }

    pub async fn find_active_by_workspace(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(&format!(
            "{} WHERE workspace_id = ? AND revoked_at IS NULL AND julianday(expires_at) > julianday('now') AND EXISTS (SELECT 1 FROM execution_processes ep WHERE ep.id = preview_leases.execution_process_id AND ep.status = 'running') ORDER BY created_at DESC",
            Self::SELECT
        ))
        .bind(workspace_id)
        .fetch_all(pool)
        .await
    }

    pub async fn revoke(pool: &SqlitePool, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE preview_leases SET revoked_at = COALESCE(revoked_at, datetime('now', 'subsec')) WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn revoke_active_by_workspace(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE preview_leases SET revoked_at = datetime('now', 'subsec') WHERE workspace_id = ? AND revoked_at IS NULL")
            .bind(workspace_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        self.revoked_at.is_some() || self.expires_at <= now
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revoked_or_past_deadline_is_expired() {
        let now = Utc::now();
        let mut lease = PreviewLease {
            id: Uuid::new_v4(),
            token_digest: "digest".into(),
            workspace_id: Uuid::new_v4(),
            execution_process_id: Uuid::new_v4(),
            target_port: 4173,
            created_at: now,
            expires_at: now + chrono::Duration::seconds(1),
            revoked_at: None,
        };
        assert!(!lease.is_expired(now));
        lease.revoked_at = Some(now);
        assert!(lease.is_expired(now));
    }
}
