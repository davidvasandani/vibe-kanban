use std::collections::{HashMap, HashSet};

use chrono::Utc;
use cluster_protocol::{
    JobState, JobSummary, PROTOCOL_VERSION, QuarantineRequest, RequestAuthority, TerminalState,
};
use db::{
    DBService,
    models::{
        execution_process::{ExecutionProcess, ExecutionProcessStatus},
        execution_worker_job::{ExecutionWorkerDispatchState, ExecutionWorkerJob},
        worker_node::{WorkerNode, WorkerNodeStatus},
    },
};
use thiserror::Error;
use uuid::Uuid;

use super::{ClusterConfig, WorkerClient, WorkerClientError};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReconciliationReport {
    pub workers_reached: usize,
    pub workers_unreachable: Vec<Uuid>,
    pub jobs_reconciled: usize,
    pub jobs_missing: usize,
    pub jobs_quarantined: usize,
    pub conflicts: usize,
    /// Terminal worker jobs whose process row is still running. Their
    /// evidence is left for the re-attached tracker to finalize.
    pub jobs_deferred: usize,
}

#[derive(Debug, Error)]
pub enum ReconciliationError {
    #[error("cluster coordinator ID is not configured")]
    MissingCoordinatorId,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Clone)]
pub struct ExecutionReconciler {
    db: DBService,
    client: WorkerClient,
    coordinator_id: Uuid,
}

impl ExecutionReconciler {
    pub fn new(
        db: DBService,
        client: WorkerClient,
        config: &ClusterConfig,
    ) -> Result<Self, ReconciliationError> {
        Ok(Self {
            db,
            client,
            coordinator_id: config
                .coordinator_id
                .ok_or(ReconciliationError::MissingCoordinatorId)?,
        })
    }

    pub async fn reconcile(&self) -> Result<ReconciliationReport, ReconciliationError> {
        let mut report = ReconciliationReport::default();
        for worker in WorkerNode::fetch_all(&self.db.pool).await? {
            if worker.status == WorkerNodeStatus::Offline {
                continue;
            }
            let inventory = match self.client.inventory(worker.id).await {
                Ok(inventory) => inventory,
                Err(error) => {
                    tracing::warn!(worker_node_id = %worker.id, "Worker inventory unavailable: {error}");
                    report.workers_unreachable.push(worker.id);
                    WorkerNode::mark_offline(&self.db.pool, worker.id).await?;
                    continue;
                }
            };
            report.workers_reached += 1;
            self.reconcile_worker(&worker, inventory, &mut report)
                .await?;
        }
        report.workers_unreachable.sort();
        Ok(report)
    }

    async fn reconcile_worker(
        &self,
        worker: &WorkerNode,
        inventory: Vec<JobSummary>,
        report: &mut ReconciliationReport,
    ) -> Result<(), ReconciliationError> {
        let expected = ExecutionWorkerJob::find_nonterminal_for_worker(&self.db.pool, worker.id)
            .await?
            .into_iter()
            .map(|job| (job.execution_process_id, job))
            .collect::<HashMap<_, _>>();
        let mut reported_ids = HashSet::new();

        for summary in inventory {
            reported_ids.insert(summary.execution_id);
            let Some(known) =
                ExecutionWorkerJob::find_by_execution_id(&self.db.pool, summary.execution_id)
                    .await?
            else {
                if self.quarantine_unknown(worker.id, &summary).await.is_ok() {
                    report.jobs_quarantined += 1;
                }
                continue;
            };
            if known.worker_node_id != worker.id
                || known.worker_job_id != summary.worker_job_id
                || known.request_digest != summary.request_digest
            {
                self.mark_indeterminate(summary.execution_id).await?;
                let _ = self.quarantine_unknown(worker.id, &summary).await;
                report.conflicts += 1;
                continue;
            }
            if !summary.state.is_terminal()
                && ExecutionProcess::find_by_id(&self.db.pool, summary.execution_id)
                    .await?
                    .is_some_and(|process| process.status != ExecutionProcessStatus::Running)
            {
                self.mark_indeterminate(summary.execution_id).await?;
                let _ = self.quarantine_unknown(worker.id, &summary).await;
                report.conflicts += 1;
                continue;
            }
            self.apply_worker_evidence(&summary, report).await?;
            report.jobs_reconciled += 1;
        }

        for execution_id in expected.keys() {
            if !reported_ids.contains(execution_id) {
                self.mark_indeterminate(*execution_id).await?;
                report.jobs_missing += 1;
            }
        }
        Ok(())
    }

    async fn quarantine_unknown(
        &self,
        worker_node_id: Uuid,
        summary: &JobSummary,
    ) -> Result<(), WorkerClientError> {
        let request = QuarantineRequest {
            authority: self.authority(worker_node_id, summary.execution_id),
            execution_id: summary.execution_id,
            reason: "worker job is unknown or conflicts with coordinator state".into(),
        };
        self.client.quarantine(worker_node_id, &request).await?;
        Ok(())
    }

    async fn apply_worker_evidence(
        &self,
        summary: &JobSummary,
        report: &mut ReconciliationReport,
    ) -> Result<(), ReconciliationError> {
        ExecutionWorkerJob::observe_worker_sequence(
            &self.db.pool,
            summary.execution_id,
            summary.last_sequence as i64,
        )
        .await?;
        let worker_state = dispatch_state(&summary.state);
        if summary.state.is_terminal() {
            let Some(evidence) = summary.terminal.as_ref() else {
                self.mark_indeterminate(summary.execution_id).await?;
                report.conflicts += 1;
                return Ok(());
            };
            let process_state = process_state(&evidence.state);
            let process = ExecutionProcess::find_by_id(&self.db.pool, summary.execution_id).await?;
            if let Some(process) = &process
                && process.status == ExecutionProcessStatus::Running
            {
                // The job finished while this coordinator was down, and its
                // row was handed off still running. Writing the evidence here
                // would skip finalization (commit, next action, queued
                // follow-up, notification) and drop the unacknowledged final
                // output. The re-attached tracker replays the terminal event
                // from the persisted cursor and finalizes through the normal
                // path instead.
                report.jobs_deferred += 1;
                return Ok(());
            }
            if let Some(process) = &process
                && process.status != process_state
            {
                self.mark_indeterminate(summary.execution_id).await?;
                report.conflicts += 1;
                return Ok(());
            }
            let evidence_json = serde_json::to_value(evidence).ok();
            ExecutionWorkerJob::update_state(
                &self.db.pool,
                summary.execution_id,
                worker_state,
                evidence_json.as_ref(),
                Some(evidence.observed_at),
            )
            .await?;
            ExecutionProcess::update_completion(
                &self.db.pool,
                summary.execution_id,
                process_state,
                evidence.exit_code.map(i64::from),
            )
            .await?;
        } else {
            ExecutionWorkerJob::update_state(
                &self.db.pool,
                summary.execution_id,
                worker_state,
                None,
                None,
            )
            .await?;
        }
        Ok(())
    }

    async fn mark_indeterminate(&self, execution_id: Uuid) -> Result<(), sqlx::Error> {
        ExecutionWorkerJob::update_state(
            &self.db.pool,
            execution_id,
            ExecutionWorkerDispatchState::Indeterminate,
            None,
            Some(Utc::now()),
        )
        .await?;
        ExecutionProcess::update_completion(
            &self.db.pool,
            execution_id,
            ExecutionProcessStatus::Indeterminate,
            None,
        )
        .await
    }

    fn authority(&self, worker_node_id: Uuid, correlation_id: Uuid) -> RequestAuthority {
        RequestAuthority {
            protocol_version: PROTOCOL_VERSION,
            coordinator_id: self.coordinator_id,
            worker_node_id,
            correlation_id,
            issued_at: Utc::now(),
            nonce: Uuid::new_v4().to_string(),
        }
    }
}

fn dispatch_state(state: &JobState) -> ExecutionWorkerDispatchState {
    match state {
        JobState::Accepted => ExecutionWorkerDispatchState::Accepted,
        JobState::Starting => ExecutionWorkerDispatchState::Starting,
        JobState::Running => ExecutionWorkerDispatchState::Running,
        JobState::Cancelling => ExecutionWorkerDispatchState::Cancelling,
        JobState::Completed => ExecutionWorkerDispatchState::Completed,
        JobState::Failed => ExecutionWorkerDispatchState::Failed,
        JobState::Killed => ExecutionWorkerDispatchState::Killed,
        JobState::Interrupted => ExecutionWorkerDispatchState::Interrupted,
        JobState::Indeterminate => ExecutionWorkerDispatchState::Indeterminate,
        JobState::Quarantined => ExecutionWorkerDispatchState::Quarantined,
    }
}

fn process_state(state: &TerminalState) -> ExecutionProcessStatus {
    match state {
        TerminalState::Completed => ExecutionProcessStatus::Completed,
        TerminalState::Failed => ExecutionProcessStatus::Failed,
        TerminalState::Killed => ExecutionProcessStatus::Killed,
        TerminalState::Interrupted => ExecutionProcessStatus::Interrupted,
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;
    use serde_json::json;
    use sqlx::{sqlite::SqlitePoolOptions, types::Json};

    use super::*;

    #[test]
    fn protocol_states_map_without_inventing_completion() {
        assert_eq!(
            dispatch_state(&JobState::Running),
            ExecutionWorkerDispatchState::Running
        );
        assert_eq!(
            dispatch_state(&JobState::Indeterminate),
            ExecutionWorkerDispatchState::Indeterminate
        );
        assert_eq!(
            process_state(&TerminalState::Killed),
            ExecutionProcessStatus::Killed
        );
    }

    #[tokio::test]
    async fn missing_worker_job_becomes_indeterminate() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&pool)
            .await
            .unwrap();
        let execution_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO execution_processes
               (id, session_id, run_reason, executor_action, status,
                started_at, created_at, updated_at)
               VALUES (?, ?, 'codingagent', '{}', 'running',
                       datetime('now'), datetime('now'), datetime('now'))"#,
        )
        .bind(execution_id)
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .unwrap();
        let worker_id = Uuid::new_v4();
        ExecutionWorkerJob::create_pending(&pool, execution_id, worker_id, "digest")
            .await
            .unwrap();

        let config = ClusterConfig {
            coordinator_id: Some(Uuid::new_v4()),
            ..Default::default()
        };
        let reconciler = ExecutionReconciler::new(
            DBService { pool: pool.clone() },
            WorkerClient::new(vec![], SigningKey::from_bytes(&[9; 32])).unwrap(),
            &config,
        )
        .unwrap();
        let worker = WorkerNode {
            id: worker_id,
            hostname: "think3".into(),
            status: WorkerNodeStatus::Online,
            worker_version: "1".into(),
            vibe_version: "1".into(),
            capabilities: Json(json!({})),
            resource_snapshot: Json(json!({})),
            labels: Json(json!({})),
            mount_status: db::models::worker_node::WorkerMountStatus::Healthy,
            mount_message: None,
            last_heartbeat_at: Some(Utc::now()),
            lease_expires_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let mut report = ReconciliationReport::default();
        reconciler
            .reconcile_worker(&worker, vec![], &mut report)
            .await
            .unwrap();

        assert_eq!(report.jobs_missing, 1);
        assert_eq!(
            ExecutionProcess::find_by_id(&pool, execution_id)
                .await
                .unwrap()
                .unwrap()
                .status,
            ExecutionProcessStatus::Indeterminate
        );
    }

    struct Fixture {
        pool: sqlx::SqlitePool,
        reconciler: ExecutionReconciler,
        worker: WorkerNode,
        execution_id: Uuid,
        worker_job_id: Uuid,
    }

    async fn accepted_job_fixture(process_status: &str) -> Fixture {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&pool)
            .await
            .unwrap();
        let execution_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO execution_processes
               (id, session_id, run_reason, executor_action, status,
                started_at, created_at, updated_at)
               VALUES (?, ?, 'codingagent', '{}', ?,
                       datetime('now'), datetime('now'), datetime('now'))"#,
        )
        .bind(execution_id)
        .bind(Uuid::new_v4())
        .bind(process_status)
        .execute(&pool)
        .await
        .unwrap();
        let worker_id = Uuid::new_v4();
        let worker_job_id = Uuid::new_v4();
        ExecutionWorkerJob::create_pending(&pool, execution_id, worker_id, "digest")
            .await
            .unwrap();
        assert!(
            ExecutionWorkerJob::record_acceptance(&pool, execution_id, worker_job_id, 0)
                .await
                .unwrap()
        );
        let config = ClusterConfig {
            coordinator_id: Some(Uuid::new_v4()),
            ..Default::default()
        };
        let reconciler = ExecutionReconciler::new(
            DBService { pool: pool.clone() },
            WorkerClient::new(vec![], SigningKey::from_bytes(&[9; 32])).unwrap(),
            &config,
        )
        .unwrap();
        let worker = WorkerNode {
            id: worker_id,
            hostname: "think3".into(),
            status: WorkerNodeStatus::Online,
            worker_version: "1".into(),
            vibe_version: "1".into(),
            capabilities: Json(json!({})),
            resource_snapshot: Json(json!({})),
            labels: Json(json!({})),
            mount_status: db::models::worker_node::WorkerMountStatus::Healthy,
            mount_message: None,
            last_heartbeat_at: Some(Utc::now()),
            lease_expires_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        Fixture {
            pool,
            reconciler,
            worker,
            execution_id,
            worker_job_id,
        }
    }

    fn completed_summary(fixture: &Fixture, last_sequence: u64) -> JobSummary {
        JobSummary {
            execution_id: fixture.execution_id,
            worker_job_id: fixture.worker_job_id,
            workspace_id: Uuid::new_v4(),
            request_digest: "digest".into(),
            state: JobState::Completed,
            last_sequence,
            terminal: Some(cluster_protocol::TerminalEvidence {
                state: TerminalState::Completed,
                exit_code: Some(0),
                signal: None,
                observed_at: Utc::now(),
            }),
        }
    }

    #[tokio::test]
    async fn terminal_job_with_running_row_is_left_for_the_reattached_tracker() {
        let fixture = accepted_job_fixture("running").await;
        let mut report = ReconciliationReport::default();
        fixture
            .reconciler
            .reconcile_worker(
                &fixture.worker,
                vec![completed_summary(&fixture, 42)],
                &mut report,
            )
            .await
            .unwrap();

        assert_eq!(report.jobs_deferred, 1);
        assert_eq!(report.conflicts, 0);
        let process = ExecutionProcess::find_by_id(&fixture.pool, fixture.execution_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            process.status,
            ExecutionProcessStatus::Running,
            "finalization belongs to the tracker, not to boot reconciliation"
        );
        let job = ExecutionWorkerJob::find_by_execution_id(&fixture.pool, fixture.execution_id)
            .await
            .unwrap()
            .unwrap();
        assert!(
            !job.dispatch_state.is_terminal(),
            "a non-terminal dispatch state is what makes the row re-attachable"
        );
        assert_eq!(
            job.worker_last_sequence, 42,
            "the observed worker sequence is still recorded"
        );
    }

    #[tokio::test]
    async fn terminal_job_with_matching_terminal_row_is_still_applied() {
        let fixture = accepted_job_fixture("completed").await;
        let mut report = ReconciliationReport::default();
        fixture
            .reconciler
            .reconcile_worker(
                &fixture.worker,
                vec![completed_summary(&fixture, 7)],
                &mut report,
            )
            .await
            .unwrap();

        assert_eq!(report.jobs_deferred, 0);
        assert_eq!(report.conflicts, 0);
        let job = ExecutionWorkerJob::find_by_execution_id(&fixture.pool, fixture.execution_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(job.dispatch_state, ExecutionWorkerDispatchState::Completed);
    }
}
