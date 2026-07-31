use chrono::{DateTime, Utc};
use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};
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
        let mut tx = pool.begin().await?;
        let current = Self::find_in_transaction(&mut tx, repo_id).await?;
        if current
            .as_ref()
            .is_some_and(|lock| lock.lease_expires_at > now && lock.operation_id != operation_id)
        {
            return Ok(None);
        }

        let generation = current.map_or(1, |lock| lock.generation + 1);
        sqlx::query(
            r#"
            INSERT INTO repository_admin_locks (
                repo_id, generation, operation_id, acquired_at, lease_expires_at
            ) VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(repo_id) DO UPDATE SET
                generation = excluded.generation,
                operation_id = excluded.operation_id,
                acquired_at = excluded.acquired_at,
                lease_expires_at = excluded.lease_expires_at
            "#,
        )
        .bind(repo_id)
        .bind(generation)
        .bind(operation_id)
        .bind(now)
        .bind(lease_expires_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(Some(Self {
            repo_id,
            generation,
            operation_id,
            acquired_at: now,
            lease_expires_at,
        }))
    }

    async fn find_in_transaction(
        tx: &mut Transaction<'_, Sqlite>,
        repo_id: Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            r#"
            SELECT repo_id, generation, operation_id, acquired_at, lease_expires_at
            FROM repository_admin_locks
            WHERE repo_id = ?
            "#,
        )
        .bind(repo_id)
        .fetch_optional(&mut **tx)
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
