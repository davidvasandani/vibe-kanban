//! Durable child ownership and immutable completion outbox. The existing issue
//! comment delivery ledger owns dispatch once the outbox has been handed off.
use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct BackgroundSubtask {
    pub child_workspace_id: Uuid,
    pub child_issue_id: Uuid,
    pub child_project_id: Uuid,
    pub parent_workspace_id: Uuid,
    pub parent_session_id: Uuid,
    pub parent_issue_id: Uuid,
    pub notification_id: Uuid,
    pub hop_count: i64,
    pub outcome: Option<String>,
    pub handed_off_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl BackgroundSubtask {
    pub async fn for_issue(pool: &SqlitePool, issue: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM background_subtasks WHERE child_issue_id = ?")
            .bind(issue)
            .fetch_optional(pool)
            .await
    }

    /// One atomic write enforces fanout and depth, even for concurrent starts.
    pub async fn register(&self, pool: &SqlitePool) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "INSERT INTO background_subtasks
             (child_workspace_id, child_issue_id, child_project_id, parent_workspace_id,
              parent_session_id, parent_issue_id, notification_id, hop_count, created_at)
             SELECT ?, ?, ?, ?, ?, ?, ?, ?, ?
             WHERE (SELECT count(*) FROM background_subtasks
                    WHERE parent_workspace_id = ? AND handed_off_at IS NULL) < 5
               AND (SELECT count(*) FROM background_subtasks WHERE created_at >= ?) < 20
               AND NOT EXISTS (SELECT 1 FROM background_subtasks WHERE child_workspace_id = ?)
             ON CONFLICT(child_issue_id) DO NOTHING",
        )
        .bind(self.child_workspace_id)
        .bind(self.child_issue_id)
        .bind(self.child_project_id)
        .bind(self.parent_workspace_id)
        .bind(self.parent_session_id)
        .bind(self.parent_issue_id)
        .bind(self.notification_id)
        .bind(self.hop_count)
        .bind(self.created_at)
        .bind(self.parent_workspace_id)
        .bind(Utc::now() - chrono::Duration::hours(1))
        .bind(self.parent_workspace_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn pending(pool: &SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as(
            "SELECT * FROM background_subtasks WHERE handed_off_at IS NULL ORDER BY created_at",
        )
        .fetch_all(pool)
        .await
    }

    pub async fn record_outcome(
        &self,
        pool: &SqlitePool,
        outcome: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE background_subtasks SET outcome = ? WHERE child_workspace_id = ? AND outcome IS NULL")
            .bind(outcome).bind(self.child_workspace_id).execute(pool).await?;
        Ok(())
    }

    pub async fn hand_off(&self, pool: &SqlitePool) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE background_subtasks SET handed_off_at = ?, last_error = NULL WHERE child_workspace_id = ?")
            .bind(Utc::now()).bind(self.child_workspace_id).execute(pool).await?;
        Ok(())
    }

    pub async fn record_error(&self, pool: &SqlitePool, error: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE background_subtasks SET last_error = ? WHERE child_workspace_id = ?")
            .bind(error)
            .bind(self.child_workspace_id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(parent_workspace_id: Uuid) -> BackgroundSubtask {
        BackgroundSubtask {
            child_workspace_id: Uuid::new_v4(),
            child_issue_id: Uuid::new_v4(),
            child_project_id: Uuid::new_v4(),
            parent_workspace_id,
            parent_session_id: Uuid::new_v4(),
            parent_issue_id: Uuid::new_v4(),
            notification_id: Uuid::new_v4(),
            hop_count: 1,
            outcome: None,
            handed_off_at: None,
            last_error: None,
            created_at: Utc::now(),
        }
    }

    async fn pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::raw_sql(include_str!(
            "../../migrations/20261023000000_background_subtasks.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn background_subtask_replay_retains_identity_and_immutable_outcome() {
        let pool = pool().await;
        let first = row(Uuid::new_v4());
        assert!(first.register(&pool).await.unwrap());
        let mut replay = first.clone();
        replay.child_workspace_id = Uuid::new_v4();
        replay.notification_id = Uuid::new_v4();
        assert!(!replay.register(&pool).await.unwrap());
        first.record_outcome(&pool, "first outcome").await.unwrap();
        first
            .record_outcome(&pool, "changed outcome")
            .await
            .unwrap();
        let saved = BackgroundSubtask::for_issue(&pool, first.child_issue_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.notification_id, first.notification_id);
        assert_eq!(saved.child_workspace_id, first.child_workspace_id);
        assert_eq!(saved.outcome.as_deref(), Some("first outcome"));
        assert_eq!(BackgroundSubtask::pending(&pool).await.unwrap().len(), 1);
        first.hand_off(&pool).await.unwrap();
        assert!(BackgroundSubtask::pending(&pool).await.unwrap().is_empty());
        assert!(!first.register(&pool).await.unwrap());
    }

    #[tokio::test]
    async fn background_subtask_launch_rate_is_global_and_durable() {
        let pool = pool().await;
        for _ in 0..20 {
            let task = row(Uuid::new_v4());
            assert!(task.register(&pool).await.unwrap());
            task.hand_off(&pool).await.unwrap();
        }
        assert!(!row(Uuid::new_v4()).register(&pool).await.unwrap());
        sqlx::query("UPDATE background_subtasks SET created_at = ?")
            .bind(Utc::now() - chrono::Duration::hours(2))
            .execute(&pool)
            .await
            .unwrap();
        assert!(row(Uuid::new_v4()).register(&pool).await.unwrap());
    }

    #[tokio::test]
    async fn background_subtask_fanout_and_depth_are_atomic() {
        let pool = pool().await;
        let parent = Uuid::new_v4();
        let first = row(parent);
        assert!(first.register(&pool).await.unwrap());
        assert!(!row(first.child_workspace_id).register(&pool).await.unwrap());
        for _ in 0..4 {
            assert!(row(parent).register(&pool).await.unwrap());
        }
        assert!(!row(parent).register(&pool).await.unwrap());
        first.hand_off(&pool).await.unwrap();
        assert!(row(parent).register(&pool).await.unwrap());
        // Completing a child must not let it start a new delegation tree.
        assert!(!row(first.child_workspace_id).register(&pool).await.unwrap());
    }
}
