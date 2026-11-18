//! Consumer for automatic capacity backoff (VK constitution LX): when a
//! coding-agent turn ends with a provider capacity error, schedule a durable
//! wake-up that resumes the session after an exponentially growing delay.
//!
//! The policy lives in `services::services::capacity_backoff`. This module
//! only gathers evidence and writes the schedule; the shared issue-comment
//! dispatcher delivers it, and cancels it if the session has moved on by then
//! (`wakeups::capacity_retry_superseded`).

use std::{future::Future, time::Duration};

use chrono::{DateTime, Utc};
use db::models::{
    execution_process::{ExecutionProcess, ExecutionProcessStatus},
    session::Session,
    session_turn_boundary::SessionTurnBoundary,
    wakeup::Wakeup,
};
use deployment::Deployment;
use services::services::{
    capacity_backoff::{
        backoff_delay, next_attempt, retry_message, turn_ended_with_capacity_error, with_jitter,
    },
    config::CapacityBackoffConfig,
    container::ContainerService,
};
use sqlx::SqlitePool;
use tokio::sync::broadcast::error::RecvError;
use uuid::Uuid;

use crate::DeploymentImpl;

/// Finalization and log normalization are separate pipelines: while the turn's
/// live log store still exists its entries may be incomplete, and on a worker
/// the last lines can reach this host after the turn is finalized. The store
/// is removed once the log is settled, so that is what is waited for.
const LOG_SETTLE_POLL: Duration = Duration::from_millis(500);
/// Upper bound on that wait. Past it the turn is judged on what is readable:
/// a trailing capacity error is still evidence, and its absence means no retry.
const LOG_SETTLE_DEADLINE: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleOutcome {
    Scheduled {
        attempt: u32,
        fire_at: DateTime<Utc>,
    },
    /// The feature is turned off.
    Disabled,
    /// The chain used all its attempts; nothing more is scheduled.
    Exhausted { previous_attempt: u32 },
    /// A retry for this turn already exists, or the session or workspace is
    /// gone, archived or without a worktree.
    NotScheduled,
}

/// Subscribe to turn boundaries for the lifetime of the process. Call before
/// boot can start (and finish) a coding-agent turn: a broadcast sent with no
/// receiver is lost.
pub fn spawn(deployment: &DeploymentImpl) {
    let Some(sender) = deployment.container().turn_finalized_sender() else {
        return;
    };
    let mut events = sender.subscribe();
    let deployment = deployment.clone();
    tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(event) => {
                    // Off the receive loop: reading one turn's log must not
                    // delay another session's event.
                    let deployment = deployment.clone();
                    tokio::spawn(async move {
                        if let Err(error) = handle(&deployment, event.session_id).await {
                            tracing::warn!(
                                session_id = %event.session_id,
                                "Capacity backoff could not inspect the finished turn: {error}"
                            );
                        }
                    });
                }
                Err(RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "Capacity backoff dropped turn-finalized events");
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
}

async fn handle(deployment: &DeploymentImpl, session_id: Uuid) -> Result<(), sqlx::Error> {
    // Read when acting, so the Settings toggle needs no restart.
    let config = deployment.config().read().await.capacity_backoff.clone();
    if !config.enabled {
        return Ok(());
    }
    let pool = &deployment.db().pool;
    let Some(turn_id) = finalized_latest_turn(pool, session_id).await? else {
        return Ok(());
    };

    let container = deployment.container();
    let settled = wait_until_settled(
        || async { container.get_msg_store_by_id(&turn_id).await.is_some() },
        LOG_SETTLE_POLL,
        LOG_SETTLE_DEADLINE,
    )
    .await;
    if !settled {
        tracing::debug!(
            %session_id,
            execution_id = %turn_id,
            "Capacity backoff: the turn's log did not settle in time; judging what is readable"
        );
    }
    // `None` means the log could not be read: no evidence, no retry.
    let Some(entries) = deployment.container().normalized_entries(&turn_id).await else {
        return Ok(());
    };
    if !turn_ended_with_capacity_error(&entries) {
        return Ok(());
    }

    let outcome = schedule(
        pool,
        &config,
        session_id,
        turn_id,
        Utc::now(),
        rand::random::<f64>(),
    )
    .await?;
    match outcome {
        ScheduleOutcome::Scheduled { attempt, fire_at } => tracing::info!(
            %session_id,
            execution_id = %turn_id,
            attempt,
            max_attempts = config.max_attempts,
            %fire_at,
            "Model at capacity: scheduled an automatic retry"
        ),
        ScheduleOutcome::Exhausted { previous_attempt } => tracing::warn!(
            %session_id,
            execution_id = %turn_id,
            previous_attempt,
            max_attempts = config.max_attempts,
            "Model still at capacity after the last automatic retry; giving up"
        ),
        ScheduleOutcome::NotScheduled => tracing::info!(
            %session_id,
            execution_id = %turn_id,
            "Model at capacity: no retry scheduled (already scheduled, or the target is unavailable)"
        ),
        ScheduleOutcome::Disabled => {}
    }
    Ok(())
}

/// Wait until `is_live` reports false (the log is settled), polling every
/// `poll` for at most `deadline`. Returns whether it settled.
async fn wait_until_settled<F, Fut>(mut is_live: F, poll: Duration, deadline: Duration) -> bool
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    let give_up = tokio::time::Instant::now() + deadline;
    loop {
        if !is_live().await {
            return true;
        }
        if tokio::time::Instant::now() >= give_up {
            return false;
        }
        tokio::time::sleep(poll).await;
    }
}

/// The session's latest coding-agent turn, when it genuinely ended (completed
/// or failed) and is the turn `finalize_task` recorded as the boundary. The
/// event alone must not identify the turn: a delayed event for an older turn
/// would otherwise act on a newer one. A provider error ends a Codex turn as
/// "completed", so both statuses are inspected; user stops, restarts and
/// indeterminate exits never are.
async fn finalized_latest_turn(
    pool: &SqlitePool,
    session_id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    let Some((turn_id, status, _)) =
        ExecutionProcess::latest_coding_agent_turn_for_session(pool, session_id).await?
    else {
        return Ok(None);
    };
    if !matches!(
        status,
        ExecutionProcessStatus::Completed | ExecutionProcessStatus::Failed
    ) {
        return Ok(None);
    }
    if SessionTurnBoundary::finalized_turn(pool, session_id).await? != Some(turn_id) {
        return Ok(None);
    }
    Ok(Some(turn_id))
}

/// Write the retry for `failed_turn`, which ended with a capacity error.
/// `jitter` is a random value in `[0, 1]`.
pub async fn schedule(
    pool: &SqlitePool,
    config: &CapacityBackoffConfig,
    session_id: Uuid,
    failed_turn: Uuid,
    now: DateTime<Utc>,
    jitter: f64,
) -> Result<ScheduleOutcome, sqlx::Error> {
    if !config.enabled {
        return Ok(ScheduleOutcome::Disabled);
    }
    // A failure of a turn that a retry started continues that retry's chain.
    let previous = Wakeup::capacity_attempt_for_execution(pool, failed_turn)
        .await?
        .map(|attempt| u32::try_from(attempt).unwrap_or(u32::MAX));
    let Some(attempt) = next_attempt(previous, config.max_attempts) else {
        return Ok(ScheduleOutcome::Exhausted {
            previous_attempt: previous.unwrap_or(0),
        });
    };
    let Some(session) = Session::find_by_id(pool, session_id).await? else {
        return Ok(ScheduleOutcome::NotScheduled);
    };
    let delay = with_jitter(
        backoff_delay(
            attempt,
            Duration::from_secs(config.base_delay_secs.into()),
            Duration::from_secs(config.max_delay_secs.into()),
        ),
        jitter,
    );
    let fire_at = now + chrono::Duration::from_std(delay).unwrap_or(chrono::Duration::MAX);
    let row = Wakeup {
        id: Uuid::new_v4(),
        workspace_id: session.workspace_id,
        session_id,
        fire_at,
        prompt: retry_message(attempt, config.max_attempts),
        poller_id: None,
        receipt_path: None,
        trigger_context: None,
        created_at: now,
        capacity_retry_of: Some(failed_turn),
        capacity_attempt: Some(attempt.into()),
    };
    Ok(if row.insert_capacity_retry(pool).await? {
        ScheduleOutcome::Scheduled { attempt, fire_at }
    } else {
        ScheduleOutcome::NotScheduled
    })
}

#[cfg(test)]
mod tests {
    use db::models::{
        execution_process::{CreateExecutionProcess, ExecutionProcessRunReason},
        session::CreateSession,
        workspace::{CreateWorkspace, Workspace},
    };
    use executors::{
        actions::{
            ExecutorAction, ExecutorActionType, coding_agent_follow_up::CodingAgentFollowUpRequest,
        },
        executors::BaseCodingAgent,
        profile::ExecutorConfig,
    };

    use super::*;

    async fn fixture() -> (SqlitePool, Uuid, Uuid) {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
        let workspace_id = Uuid::new_v4();
        Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "capacity".into(),
                name: None,
            },
            workspace_id,
        )
        .await
        .unwrap();
        let session_id = Uuid::new_v4();
        Session::create(
            &pool,
            &CreateSession {
                executor: None,
                name: None,
            },
            session_id,
            workspace_id,
        )
        .await
        .unwrap();
        (pool, workspace_id, session_id)
    }

    async fn turn(pool: &SqlitePool, session_id: Uuid, status: ExecutionProcessStatus) -> Uuid {
        let id = Uuid::new_v4();
        let action = ExecutorAction::new(
            ExecutorActionType::CodingAgentFollowUpRequest(CodingAgentFollowUpRequest {
                prompt: "work".into(),
                session_id: "agent-session".into(),
                reset_to_message_id: None,
                executor_config: ExecutorConfig::new(BaseCodingAgent::Codex),
                working_dir: None,
            }),
            None,
        );
        ExecutionProcess::create(
            pool,
            &CreateExecutionProcess {
                session_id,
                executor_action: action,
                run_reason: ExecutionProcessRunReason::CodingAgent,
            },
            id,
            &[],
        )
        .await
        .unwrap();
        if status != ExecutionProcessStatus::Running {
            ExecutionProcess::update_completion(pool, id, status, Some(0))
                .await
                .unwrap();
        }
        id
    }

    /// Mark the pending retry for `failed_turn` as having started `started`.
    async fn deliver(pool: &SqlitePool, failed_turn: Uuid, started: Uuid) {
        let changed = sqlx::query("UPDATE issue_comment_deliveries SET state = 'delivered', execution_process_id = ? WHERE comment_id = (SELECT id FROM wakeups WHERE capacity_retry_of = ?)")
            .bind(started)
            .bind(failed_turn)
            .execute(pool)
            .await
            .unwrap()
            .rows_affected();
        assert_eq!(changed, 1);
    }

    fn config() -> CapacityBackoffConfig {
        CapacityBackoffConfig::default()
    }

    #[tokio::test(start_paused = true)]
    async fn waits_for_the_log_to_settle_but_not_forever() {
        use std::sync::atomic::{AtomicU32, Ordering};

        // Already settled: no waiting at all.
        let started = tokio::time::Instant::now();
        assert!(wait_until_settled(|| async { false }, LOG_SETTLE_POLL, LOG_SETTLE_DEADLINE).await);
        assert_eq!(started.elapsed(), Duration::ZERO);

        // Normalization lags well past one second, then settles.
        let polls = AtomicU32::new(0);
        let started = tokio::time::Instant::now();
        assert!(
            wait_until_settled(
                || async { polls.fetch_add(1, Ordering::SeqCst) < 10 },
                LOG_SETTLE_POLL,
                LOG_SETTLE_DEADLINE
            )
            .await
        );
        assert_eq!(started.elapsed(), Duration::from_secs(5));

        // Never settles: bounded by the deadline.
        let started = tokio::time::Instant::now();
        assert!(!wait_until_settled(|| async { true }, LOG_SETTLE_POLL, LOG_SETTLE_DEADLINE).await);
        assert_eq!(started.elapsed(), LOG_SETTLE_DEADLINE);
    }

    #[tokio::test]
    async fn first_failure_schedules_one_retry_after_the_base_delay() {
        let (pool, workspace, session) = fixture().await;
        let failed = turn(&pool, session, ExecutionProcessStatus::Completed).await;
        let now = Utc::now();

        let outcome = schedule(&pool, &config(), session, failed, now, 0.0)
            .await
            .unwrap();
        assert_eq!(
            outcome,
            ScheduleOutcome::Scheduled {
                attempt: 1,
                fire_at: now + chrono::Duration::seconds(60),
            }
        );
        // A duplicate event for the same turn adds nothing.
        assert_eq!(
            schedule(&pool, &config(), session, failed, now, 0.0)
                .await
                .unwrap(),
            ScheduleOutcome::NotScheduled
        );

        let rows = Wakeup::list(&pool, workspace).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].wakeup.capacity_retry_of, Some(failed));
        assert_eq!(rows[0].wakeup.capacity_attempt, Some(1));
        assert!(rows[0].wakeup.prompt.contains("attempt 1 of 10"));
    }

    #[tokio::test]
    async fn jitter_delays_the_retry_by_at_most_a_fifth() {
        let (pool, _, session) = fixture().await;
        let failed = turn(&pool, session, ExecutionProcessStatus::Completed).await;
        let now = Utc::now();
        let ScheduleOutcome::Scheduled { fire_at, .. } =
            schedule(&pool, &config(), session, failed, now, 1.0)
                .await
                .unwrap()
        else {
            panic!("expected a retry");
        };
        assert_eq!(fire_at, now + chrono::Duration::seconds(72));
    }

    #[tokio::test]
    async fn consecutive_failures_double_the_delay_then_give_up() {
        let (pool, _, session) = fixture().await;
        let now = Utc::now();
        let mut failed = turn(&pool, session, ExecutionProcessStatus::Completed).await;
        let mut delays = Vec::new();
        for expected_attempt in 1..=10u32 {
            let ScheduleOutcome::Scheduled { attempt, fire_at } =
                schedule(&pool, &config(), session, failed, now, 0.0)
                    .await
                    .unwrap()
            else {
                panic!("attempt {expected_attempt} should be scheduled");
            };
            assert_eq!(attempt, expected_attempt);
            delays.push((fire_at - now).num_seconds());
            // The retry fires, starts a turn, and that turn fails the same way.
            let retried = turn(&pool, session, ExecutionProcessStatus::Completed).await;
            deliver(&pool, failed, retried).await;
            failed = retried;
        }
        assert_eq!(
            delays,
            [60, 120, 240, 480, 960, 1920, 3600, 3600, 3600, 3600]
        );
        assert_eq!(
            schedule(&pool, &config(), session, failed, now, 0.0)
                .await
                .unwrap(),
            ScheduleOutcome::Exhausted {
                previous_attempt: 10
            }
        );
    }

    #[tokio::test]
    async fn a_failure_of_an_unrelated_turn_starts_a_new_chain() {
        let (pool, _, session) = fixture().await;
        let now = Utc::now();
        let first = turn(&pool, session, ExecutionProcessStatus::Completed).await;
        schedule(&pool, &config(), session, first, now, 0.0)
            .await
            .unwrap();
        let retried = turn(&pool, session, ExecutionProcessStatus::Completed).await;
        deliver(&pool, first, retried).await;
        // The retry worked; much later a human-started turn hits capacity.
        let later = turn(&pool, session, ExecutionProcessStatus::Failed).await;
        assert!(matches!(
            schedule(&pool, &config(), session, later, now, 0.0)
                .await
                .unwrap(),
            ScheduleOutcome::Scheduled { attempt: 1, .. }
        ));
    }

    #[tokio::test]
    async fn disabled_or_zero_attempts_schedule_nothing() {
        let (pool, workspace, session) = fixture().await;
        let failed = turn(&pool, session, ExecutionProcessStatus::Completed).await;
        let off = CapacityBackoffConfig {
            enabled: false,
            ..config()
        };
        assert_eq!(
            schedule(&pool, &off, session, failed, Utc::now(), 0.0)
                .await
                .unwrap(),
            ScheduleOutcome::Disabled
        );
        let zero = CapacityBackoffConfig {
            max_attempts: 0,
            ..config()
        };
        assert_eq!(
            schedule(&pool, &zero, session, failed, Utc::now(), 0.0)
                .await
                .unwrap(),
            ScheduleOutcome::Exhausted {
                previous_attempt: 0
            }
        );
        assert!(Wakeup::list(&pool, workspace).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn archived_workspace_or_missing_session_is_left_alone() {
        let (pool, workspace, session) = fixture().await;
        let failed = turn(&pool, session, ExecutionProcessStatus::Completed).await;
        assert_eq!(
            schedule(&pool, &config(), Uuid::new_v4(), failed, Utc::now(), 0.0)
                .await
                .unwrap(),
            ScheduleOutcome::NotScheduled
        );
        sqlx::query("UPDATE workspaces SET archived = TRUE WHERE id = ?")
            .bind(workspace)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            schedule(&pool, &config(), session, failed, Utc::now(), 0.0)
                .await
                .unwrap(),
            ScheduleOutcome::NotScheduled
        );
        let archived: bool = sqlx::query_scalar("SELECT archived FROM workspaces WHERE id = ?")
            .bind(workspace)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(archived, "scheduling never unarchives");
    }

    #[tokio::test]
    async fn only_the_recorded_boundary_turn_is_inspected() {
        let (pool, _, session) = fixture().await;
        // No boundary recorded yet.
        let done = turn(&pool, session, ExecutionProcessStatus::Completed).await;
        assert_eq!(finalized_latest_turn(&pool, session).await.unwrap(), None);

        SessionTurnBoundary::record_finalized(&pool, session, done)
            .await
            .unwrap();
        assert_eq!(
            finalized_latest_turn(&pool, session).await.unwrap(),
            Some(done)
        );

        // A newer turn started: a delayed event for `done` must not act on it.
        let running = turn(&pool, session, ExecutionProcessStatus::Running).await;
        assert_eq!(finalized_latest_turn(&pool, session).await.unwrap(), None);

        // User stops are never inspected, even once recorded as the boundary.
        ExecutionProcess::update_completion(&pool, running, ExecutionProcessStatus::Killed, None)
            .await
            .unwrap();
        SessionTurnBoundary::record_finalized(&pool, session, running)
            .await
            .unwrap();
        assert_eq!(finalized_latest_turn(&pool, session).await.unwrap(), None);

        let failed = turn(&pool, session, ExecutionProcessStatus::Failed).await;
        SessionTurnBoundary::record_finalized(&pool, session, failed)
            .await
            .unwrap();
        assert_eq!(
            finalized_latest_turn(&pool, session).await.unwrap(),
            Some(failed)
        );
    }
}
