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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type)]
#[sqlx(type_name = "dispatch_recovery_checkpoint", rename_all = "lowercase")]
pub enum DispatchRecoveryCheckpoint {
    Initial,
    Refused,
    Ready,
    Replacement,
    Stopped,
}

#[derive(Debug, Clone, Copy)]
pub enum DispatchRecoveryStep {
    Probe,
    Verify,
    Replace,
}

#[derive(Debug, FromRow)]
pub struct DispatchRecovery {
    pub coordinator_id: Option<Uuid>,
    pub coordinator_instance_id: Option<Uuid>,
    pub recovery_checkpoint: DispatchRecoveryCheckpoint,
    pub readiness_attempts: i64,
    pub refusal_detail: Option<String>,
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
        request_digest: &str,
    ) -> Result<Self, sqlx::Error> {
        // The worker owns the final job ID, but the schema requires a unique
        // non-null value before dispatch is accepted. Use the already-unique
        // execution ID as the pending placeholder so concurrent dispatches to
        // one worker cannot collide.
        let worker_job_id = execution_process_id;
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO execution_worker_jobs (
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
        let job = Self::find_by_execution_id(pool, execution_process_id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;
        if job.worker_node_id != worker_node_id || job.request_digest != request_digest {
            return Err(sqlx::Error::Protocol(
                "existing worker dispatch does not match the retried request".into(),
            ));
        }
        Ok(job)
    }

    pub async fn create_owned_pending(
        pool: &SqlitePool,
        execution_id: Uuid,
        worker_node_id: Uuid,
        digest: &str,
        coordinator_id: Uuid,
        instance_id: Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        let inserted = sqlx::query(
            "INSERT INTO execution_worker_jobs (
                 execution_process_id, worker_node_id, worker_job_id, request_digest,
                 coordinator_id, coordinator_instance_id
             )
             SELECT ep.id, ?, ep.id, ?, ?, ?
             FROM execution_processes ep
             JOIN sessions s ON s.id = ep.session_id
             JOIN workspaces w ON w.id = s.workspace_id
             WHERE ep.id = ? AND ep.status = 'running' AND ep.dropped = 0
               AND w.archived = 0 AND w.placement_state = 'ready' AND w.worker_node_id = ?
             ON CONFLICT (execution_process_id) DO NOTHING",
        )
        .bind(worker_node_id)
        .bind(digest)
        .bind(coordinator_id)
        .bind(instance_id)
        .bind(execution_id)
        .bind(worker_node_id)
        .execute(pool)
        .await?
        .rows_affected()
            == 1;
        if inserted {
            Self::find_by_execution_id(pool, execution_id).await
        } else {
            Ok(None)
        }
    }

    pub async fn take_over_dispatch(
        pool: &SqlitePool,
        execution_id: Uuid,
        coordinator_id: Uuid,
        previous_instance_id: Uuid,
        instance_id: Uuid,
        state: ExecutionWorkerDispatchState,
    ) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query(
            "UPDATE execution_worker_jobs SET coordinator_instance_id = ?
             WHERE execution_process_id = ? AND coordinator_id = ?
               AND coordinator_instance_id = ? AND dispatch_state = ?
               AND EXISTS (SELECT 1 FROM execution_processes ep
                           WHERE ep.id = execution_process_id AND ep.status = 'running')",
        )
        .bind(instance_id)
        .bind(execution_id)
        .bind(coordinator_id)
        .bind(previous_instance_id)
        .bind(state)
        .execute(pool)
        .await?
        .rows_affected()
            == 1)
    }

    pub async fn recovery(
        pool: &SqlitePool,
        execution_id: Uuid,
    ) -> Result<DispatchRecovery, sqlx::Error> {
        sqlx::query_as(
            "SELECT coordinator_id, coordinator_instance_id, recovery_checkpoint, readiness_attempts, refusal_detail
             FROM execution_worker_jobs WHERE execution_process_id = ?",
        )
        .bind(execution_id)
        .fetch_one(pool)
        .await
    }

    pub async fn record_refusal(
        pool: &SqlitePool,
        execution_id: Uuid,
        instance_id: Uuid,
        detail: &str,
    ) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query(
            "UPDATE execution_worker_jobs
             SET recovery_checkpoint = 'refused', refusal_detail = ?
             WHERE execution_process_id = ? AND coordinator_instance_id = ?
               AND dispatch_state = 'pending' AND recovery_checkpoint = 'initial'",
        )
        .bind(detail)
        .bind(execution_id)
        .bind(instance_id)
        .execute(pool)
        .await?
        .rows_affected()
            == 1)
    }

    pub async fn claim_recovery_step(
        pool: &SqlitePool,
        execution_id: Uuid,
        instance_id: Uuid,
        probes: i64,
        step: DispatchRecoveryStep,
    ) -> Result<bool, sqlx::Error> {
        use DispatchRecoveryCheckpoint::{Ready, Refused, Replacement};
        let (from, to, increment) = match step {
            DispatchRecoveryStep::Probe => (Refused, Refused, 1),
            DispatchRecoveryStep::Verify => (Refused, Ready, 0),
            DispatchRecoveryStep::Replace => (Ready, Replacement, 0),
        };
        Ok(sqlx::query(
            "UPDATE execution_worker_jobs
             SET readiness_attempts = readiness_attempts + ?,
                 recovery_checkpoint = ?
             WHERE execution_process_id = ? AND coordinator_instance_id = ?
               AND dispatch_state = 'pending' AND recovery_checkpoint = ?
               AND readiness_attempts = ? AND (? OR readiness_attempts < 2)
               AND EXISTS (
                   SELECT 1 FROM execution_processes ep
                   JOIN sessions s ON s.id = ep.session_id
                   JOIN workspaces w ON w.id = s.workspace_id
                   WHERE ep.id = execution_process_id AND ep.status = 'running'
                     AND ep.dropped = 0 AND w.archived = 0
                     AND w.placement_state = 'ready'
                     AND w.worker_node_id = execution_worker_jobs.worker_node_id
               )",
        )
        .bind(increment)
        .bind(to)
        .bind(execution_id)
        .bind(instance_id)
        .bind(from)
        .bind(probes)
        .bind(increment == 0)
        .execute(pool)
        .await?
        .rows_affected()
            == 1)
    }

    pub async fn settle_pending_dispatch(
        pool: &SqlitePool,
        execution_id: Uuid,
        coordinator_id: Uuid,
        state: ExecutionWorkerDispatchState,
        status: super::execution_process::ExecutionProcessStatus,
        detail: &str,
    ) -> Result<bool, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let changed = sqlx::query(
            "UPDATE execution_worker_jobs
             SET dispatch_state = ?, terminal_evidence = ?, completed_at = datetime('now', 'subsec')
             WHERE execution_process_id = ? AND coordinator_id = ? AND dispatch_state = 'pending'",
        )
        .bind(state)
        .bind(Json(serde_json::json!({ "reason": detail })))
        .bind(execution_id)
        .bind(coordinator_id)
        .execute(&mut *tx)
        .await?
        .rows_affected()
            == 1;
        if changed {
            sqlx::query(
                "UPDATE execution_processes SET status = ?, completed_at = datetime('now', 'subsec')
                 WHERE id = ? AND status = 'running'",
            )
            .bind(status)
            .bind(execution_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(changed)
    }

    pub async fn stop_pending_recovery(
        pool: &SqlitePool,
        execution_id: Uuid,
        status: &super::execution_process::ExecutionProcessStatus,
    ) -> Result<bool, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let changed = sqlx::query(
            "UPDATE execution_worker_jobs
             SET recovery_checkpoint = 'stopped', dispatch_state = ?,
                 completed_at = datetime('now', 'subsec')
             WHERE execution_process_id = ? AND dispatch_state = 'pending'
               AND recovery_checkpoint IN ('refused', 'ready')",
        )
        .bind(status)
        .bind(execution_id)
        .execute(&mut *tx)
        .await?
        .rows_affected()
            == 1;
        if changed {
            sqlx::query(
                "UPDATE execution_processes SET status = ?, completed_at = datetime('now', 'subsec')
                 WHERE id = ? AND status = 'running'",
            )
            .bind(status)
            .bind(execution_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(changed)
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

    pub async fn find_nonterminal_for_worker(
        pool: &SqlitePool,
        worker_node_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(&format!(
            "{} WHERE worker_node_id = ? AND dispatch_state NOT IN \
             ('completed', 'failed', 'killed', 'interrupted', 'indeterminate', 'quarantined') \
             ORDER BY execution_process_id",
            Self::SELECT
        ))
        .bind(worker_node_id)
        .fetch_all(pool)
        .await
    }

    pub async fn has_unsafe_for_workspace(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM execution_worker_jobs ewj
            JOIN execution_processes ep ON ep.id = ewj.execution_process_id
            JOIN sessions s ON s.id = ep.session_id
            WHERE s.workspace_id = ?
              AND ewj.dispatch_state NOT IN ('completed', 'failed', 'killed')
            "#,
        )
        .bind(workspace_id)
        .fetch_one(pool)
        .await?;
        Ok(count > 0)
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

    pub async fn observe_worker_sequence(
        pool: &SqlitePool,
        execution_process_id: Uuid,
        worker_last_sequence: i64,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE execution_worker_jobs
            SET worker_last_sequence = MAX(worker_last_sequence, ?),
                updated_at = datetime('now', 'subsec')
            WHERE execution_process_id = ?
            "#,
        )
        .bind(worker_last_sequence)
        .bind(execution_process_id)
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
