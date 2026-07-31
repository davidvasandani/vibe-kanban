use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, SqlitePool, Type, types::Json};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Type, TS)]
#[sqlx(
    type_name = "execution_worker_dispatch_state",
    rename_all = "lowercase"
)]
#[serde(rename_all = "lowercase")]
#[ts(use_ts_enum)]
pub enum ExecutionWorkerDispatchState {
    Pending,
    Accepted,
    Starting,
    Running,
    Cancelling,
    Completed,
    Failed,
    Killed,
    Interrupted,
    Indeterminate,
    Quarantined,
}

impl ExecutionWorkerDispatchState {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed
                | Self::Failed
                | Self::Killed
                | Self::Interrupted
                | Self::Indeterminate
                | Self::Quarantined
        )
    }
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
pub struct ExecutionWorkerJob {
    pub execution_process_id: Uuid,
    pub worker_node_id: Uuid,
    pub worker_job_id: Uuid,
    pub request_digest: String,
    pub dispatch_state: ExecutionWorkerDispatchState,
    pub last_event_sequence: i64,
    pub worker_last_sequence: i64,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub output_complete: bool,
    #[ts(type = "unknown")]
    pub terminal_evidence: Option<Json<Value>>,
    pub dispatched_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ExecutionWorkerJob {
    const SELECT: &'static str = r#"
        SELECT execution_process_id, worker_node_id, worker_job_id,
               request_digest, dispatch_state, last_event_sequence,
               worker_last_sequence, lease_expires_at, output_complete,
               terminal_evidence, dispatched_at, accepted_at, completed_at,
               created_at, updated_at
        FROM execution_worker_jobs
    "#;

    pub async fn create_pending(
        pool: &SqlitePool,
        execution_process_id: Uuid,
        worker_node_id: Uuid,
        worker_job_id: Uuid,
        request_digest: &str,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO execution_worker_jobs (
                execution_process_id, worker_node_id, worker_job_id,
                request_digest, dispatch_state
            ) VALUES (?, ?, ?, ?, 'pending')
            "#,
        )
        .bind(execution_process_id)
        .bind(worker_node_id)
        .bind(worker_job_id)
        .bind(request_digest)
        .execute(pool)
        .await?;
        Self::find_by_execution_id(pool, execution_process_id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn find_by_execution_id(
        pool: &SqlitePool,
        execution_process_id: Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(&format!("{} WHERE execution_process_id = ?", Self::SELECT))
            .bind(execution_process_id)
            .fetch_optional(pool)
            .await
    }

    pub async fn acknowledge_sequence(
        pool: &SqlitePool,
        execution_process_id: Uuid,
        sequence: i64,
        worker_last_sequence: i64,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE execution_worker_jobs
            SET last_event_sequence = ?,
                worker_last_sequence = MAX(worker_last_sequence, ?),
                updated_at = datetime('now', 'subsec')
            WHERE execution_process_id = ?
              AND last_event_sequence <= ?
            "#,
        )
        .bind(sequence)
        .bind(worker_last_sequence)
        .bind(execution_process_id)
        .bind(sequence)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn update_state(
        pool: &SqlitePool,
        execution_process_id: Uuid,
        state: ExecutionWorkerDispatchState,
        terminal_evidence: Option<&Value>,
        completed_at: Option<DateTime<Utc>>,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE execution_worker_jobs
            SET dispatch_state = ?,
                terminal_evidence = ?,
                accepted_at = CASE
                    WHEN ? = 'accepted' AND accepted_at IS NULL
                    THEN datetime('now', 'subsec')
                    ELSE accepted_at
                END,
                completed_at = COALESCE(?, completed_at),
                updated_at = datetime('now', 'subsec')
            WHERE execution_process_id = ?
            "#,
        )
        .bind(state)
        .bind(terminal_evidence.map(Json))
        .bind(state)
        .bind(completed_at)
        .bind(execution_process_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn record_acceptance(
        pool: &SqlitePool,
        execution_process_id: Uuid,
        worker_job_id: Uuid,
        worker_last_sequence: i64,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE execution_worker_jobs
            SET worker_job_id = ?,
                dispatch_state = 'accepted',
                worker_last_sequence = MAX(worker_last_sequence, ?),
                accepted_at = COALESCE(accepted_at, datetime('now', 'subsec')),
                updated_at = datetime('now', 'subsec')
            WHERE execution_process_id = ? AND dispatch_state = 'pending'
            "#,
        )
        .bind(worker_job_id)
        .bind(worker_last_sequence)
        .bind(execution_process_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn mark_output_incomplete(
        pool: &SqlitePool,
        execution_process_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE execution_worker_jobs
            SET output_complete = 0, updated_at = datetime('now', 'subsec')
            WHERE execution_process_id = ?
            "#,
        )
        .bind(execution_process_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }
}
