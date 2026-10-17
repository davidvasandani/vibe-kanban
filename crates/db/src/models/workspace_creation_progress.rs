use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use ts_rs::TS;
use uuid::Uuid;

use super::workspace::WorkspaceCreationStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, TS)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum WorkspaceCreationPhase {
    Repositories,
    Context,
    Placement,
    Worktrees,
    Execution,
    Finalizing,
}

impl WorkspaceCreationPhase {
    fn ordinal(self) -> i32 {
        match self {
            Self::Repositories => 0,
            Self::Context => 1,
            Self::Placement => 2,
            Self::Worktrees => 3,
            Self::Execution => 4,
            Self::Finalizing => 5,
        }
    }
}

/// A last-reported boundary, not proof of process liveness. Lifecycle status
/// and phase are read together so restart/failure cannot leave an active UI.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
pub struct WorkspaceCreationProgress {
    pub workspace_id: Uuid,
    pub status: WorkspaceCreationStatus,
    pub phase: Option<WorkspaceCreationPhase>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl WorkspaceCreationProgress {
    pub async fn find(pool: &SqlitePool, workspace_id: Uuid) -> Result<Self, sqlx::Error> {
        sqlx::query_as(
            "SELECT w.id AS workspace_id, w.creation_status AS status, p.phase, p.updated_at
             FROM workspaces w LEFT JOIN workspace_creation_progress p ON p.workspace_id = w.id
             WHERE w.id = ?",
        )
        .bind(workspace_id)
        .fetch_one(pool)
        .await
    }

    async fn record(
        pool: &SqlitePool,
        workspace_id: Uuid,
        phase: WorkspaceCreationPhase,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO workspace_creation_progress (workspace_id, phase)
             SELECT id, ? FROM workspaces WHERE id = ? AND creation_status = 'running'
             ON CONFLICT(workspace_id) DO UPDATE SET
                phase = excluded.phase, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE CASE workspace_creation_progress.phase
                WHEN 'repositories' THEN 0 WHEN 'context' THEN 1 WHEN 'placement' THEN 2
                WHEN 'worktrees' THEN 3 WHEN 'execution' THEN 4 WHEN 'finalizing' THEN 5
             END < ?",
        )
        .bind(phase)
        .bind(workspace_id)
        .bind(phase.ordinal())
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Observability must never abort or replay an otherwise valid startup.
    pub async fn report(pool: &SqlitePool, workspace_id: Uuid, phase: WorkspaceCreationPhase) {
        if let Err(error) = Self::record(pool, workspace_id, phase).await {
            tracing::warn!(%workspace_id, ?phase, %error, "Could not record workspace creation progress");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::workspace::{CreateWorkspace, Workspace};

    async fn fixture() -> (SqlitePool, Uuid) {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let id = Uuid::new_v4();
        Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "vk/progress".into(),
                name: None,
            },
            id,
        )
        .await
        .unwrap();
        (pool, id)
    }

    #[tokio::test]
    async fn progress_requires_claim_and_does_not_regress() {
        let (pool, id) = fixture().await;
        WorkspaceCreationProgress::record(&pool, id, WorkspaceCreationPhase::Repositories)
            .await
            .unwrap();
        assert!(
            WorkspaceCreationProgress::find(&pool, id)
                .await
                .unwrap()
                .phase
                .is_none()
        );
        Workspace::queue_creation(&pool, id).await.unwrap();
        WorkspaceCreationProgress::record(&pool, id, WorkspaceCreationPhase::Repositories)
            .await
            .unwrap();
        assert!(
            WorkspaceCreationProgress::find(&pool, id)
                .await
                .unwrap()
                .phase
                .is_none()
        );
        Workspace::claim_creation(&pool, id).await.unwrap();
        WorkspaceCreationProgress::record(&pool, id, WorkspaceCreationPhase::Worktrees)
            .await
            .unwrap();
        let first = WorkspaceCreationProgress::find(&pool, id).await.unwrap();
        for phase in [
            WorkspaceCreationPhase::Repositories,
            WorkspaceCreationPhase::Worktrees,
        ] {
            WorkspaceCreationProgress::record(&pool, id, phase)
                .await
                .unwrap();
        }
        let reloaded = WorkspaceCreationProgress::find(&pool, id).await.unwrap();
        assert_eq!(reloaded.workspace_id, id);
        assert_eq!(reloaded.phase, Some(WorkspaceCreationPhase::Worktrees));
        assert_eq!(reloaded.updated_at, first.updated_at);
        assert_eq!(reloaded.status, WorkspaceCreationStatus::Running);
        WorkspaceCreationProgress::record(&pool, id, WorkspaceCreationPhase::Execution)
            .await
            .unwrap();
        assert_eq!(
            WorkspaceCreationProgress::find(&pool, id)
                .await
                .unwrap()
                .phase,
            Some(WorkspaceCreationPhase::Execution)
        );
        assert!(matches!(
            WorkspaceCreationProgress::find(&pool, Uuid::new_v4()).await,
            Err(sqlx::Error::RowNotFound)
        ));
    }

    #[tokio::test]
    async fn terminal_states_override_retained_phase_and_reject_late_reports() {
        for terminal in ["ready", "failed", "restart"] {
            let (pool, id) = fixture().await;
            Workspace::queue_creation(&pool, id).await.unwrap();
            Workspace::claim_creation(&pool, id).await.unwrap();
            WorkspaceCreationProgress::record(&pool, id, WorkspaceCreationPhase::Execution)
                .await
                .unwrap();
            match terminal {
                "ready" => {
                    Workspace::finish_creation(&pool, id).await.unwrap();
                }
                "failed" => {
                    Workspace::fail_creation(&pool, id, "Failed").await.unwrap();
                }
                _ => {
                    Workspace::fail_unfinished_creations(&pool).await.unwrap();
                }
            }
            WorkspaceCreationProgress::record(&pool, id, WorkspaceCreationPhase::Finalizing)
                .await
                .unwrap();
            let progress = WorkspaceCreationProgress::find(&pool, id).await.unwrap();
            assert_eq!(progress.phase, Some(WorkspaceCreationPhase::Execution));
            assert_eq!(
                progress.status,
                if terminal == "ready" {
                    WorkspaceCreationStatus::Ready
                } else {
                    WorkspaceCreationStatus::Failed
                }
            );
            sqlx::query("DELETE FROM workspaces WHERE id = ?")
                .bind(id)
                .execute(&pool)
                .await
                .unwrap();
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspace_creation_progress")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(count, 0);
        }
    }
}
