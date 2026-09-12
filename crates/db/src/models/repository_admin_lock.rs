use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct RepositoryAdminLock {
    pub repo_id: Uuid,
    pub generation: i64,
    pub operation_id: Uuid,
    pub acquired_at: DateTime<Utc>,
    pub lease_expires_at: DateTime<Utc>,
}

impl RepositoryAdminLock {
    pub async fn acquire(
        pool: &SqlitePool,
        repo_id: Uuid,
        operation_id: Uuid,
        now: DateTime<Utc>,
        lease_expires_at: DateTime<Utc>,
    ) -> Result<Option<Self>, sqlx::Error> {
        // The lease predicate is part of the write itself. A preceding SELECT
        // would allow two contenders to observe the same expired row and both
        // believe they acquired it.
        sqlx::query_as::<_, Self>(
            r#"
            INSERT INTO repository_admin_locks (
                repo_id, generation, operation_id, acquired_at, lease_expires_at
            ) VALUES (?, 1, ?, ?, ?)
            ON CONFLICT(repo_id) DO UPDATE SET
                generation = repository_admin_locks.generation + 1,
                operation_id = excluded.operation_id,
                acquired_at = excluded.acquired_at,
                lease_expires_at = excluded.lease_expires_at
            WHERE repository_admin_locks.lease_expires_at <= excluded.acquired_at
               OR repository_admin_locks.operation_id = excluded.operation_id
            RETURNING repo_id, generation, operation_id, acquired_at, lease_expires_at
            "#,
        )
        .bind(repo_id)
        .bind(operation_id)
        .bind(now)
        .bind(lease_expires_at)
        .fetch_optional(pool)
        .await
    }

    pub async fn release(
        pool: &SqlitePool,
        repo_id: Uuid,
        generation: i64,
        operation_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        // Retain the row as a fencing tombstone. Deleting it would reset the
        // next owner's generation to 1 and let an old token become ambiguous.
        let result = sqlx::query(
            r#"
            UPDATE repository_admin_locks
            SET lease_expires_at = acquired_at
            WHERE repo_id = ? AND generation = ? AND operation_id = ?
            "#,
        )
        .bind(repo_id)
        .bind(generation)
        .bind(operation_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }
}
