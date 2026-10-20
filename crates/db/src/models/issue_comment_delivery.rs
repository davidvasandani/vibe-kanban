//! Durable record of delivering one remote issue comment to one local
//! workspace session. `comment_id` is unique, so a comment is delivered to an
//! agent at most once whatever the number of requests, retries or restarts.
//!
//! State changes are conditional updates on the current state: the dispatcher
//! claims a `pending` row before starting anything, and only the claimant can
//! move a `dispatching` row on (VK constitution XII/XXVIII).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool, Type};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type, Serialize, Deserialize, TS)]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum IssueCommentDeliveryState {
    /// Accepted; waiting for the target's turn boundary or a scheduled retry.
    Pending,
    /// Claimed by the dispatcher, which is starting the follow-up.
    Dispatching,
    /// A follow-up execution carrying the comment was started.
    Delivered,
    /// Several sessions are eligible and none is established; a user must choose.
    NeedsTarget,
    /// No session may receive the comment (see `reason`).
    NoTarget,
    /// Dispatch failed after automatic retries (see `error`).
    Failed,
    /// Refused by a loop guard.
    Blocked,
    /// The comment was deleted before it was delivered.
    Cancelled,
}

impl IssueCommentDeliveryState {
    /// States a new explicit request (retry / target choice) may re-resolve.
    pub fn is_resolvable(self) -> bool {
        matches!(self, Self::NeedsTarget | Self::NoTarget | Self::Failed)
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Dispatching => "dispatching",
            Self::Delivered => "delivered",
            Self::NeedsTarget => "needs_target",
            Self::NoTarget => "no_target",
            Self::Failed => "failed",
            Self::Blocked => "blocked",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Machine-readable explanation of a delivery's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Type, Serialize, Deserialize, TS)]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum IssueCommentDeliveryReason {
    /// Pending: the target agent is running; delivered at its turn boundary.
    AgentRunning,
    /// Pending: a follow-up message is queued for the target and goes first.
    QueuedMessage,
    /// Pending: a failed attempt will be retried at `next_attempt_at`.
    RetryScheduled,
    NoLinkedWorkspace,
    /// Linked workspaces exist but none lives on this host.
    WorkspaceOtherHost,
    WorkspaceArchived,
    /// The issue is Done/Cancelled.
    IssueTerminal,
    /// Linked workspaces have no session with a coding-agent turn.
    NoSessions,
    /// The target's last turn failed or was stopped; needs an explicit choice.
    TurnStopped,
    /// An explicit target is not a session of a workspace linked to the issue.
    TargetNotLinked,
    Ambiguous,
    /// An agent's comment would resume its own session.
    SelfTarget,
    /// Agent-to-agent delivery chain exceeded the hop limit.
    LoopGuard,
    CommentDeleted,
    /// The remote server could not answer (network, auth or missing route).
    RemoteUnavailable,
    /// The current user may not read the comment's issue.
    Unauthorized,
    StartFailed,
    /// A worker received the turn but never confirmed it started; sending
    /// again could run the agent twice, so only an explicit choice retries.
    LaunchUncertain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type, Serialize, Deserialize, TS)]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum IssueCommentDeliverySource {
    Ui,
    Mcp,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
pub struct IssueCommentDelivery {
    pub id: Uuid,
    pub comment_id: Uuid,
    pub issue_id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub execution_process_id: Option<Uuid>,
    pub state: IssueCommentDeliveryState,
    pub reason: Option<IssueCommentDeliveryReason>,
    pub error: Option<String>,
    pub source: IssueCommentDeliverySource,
    pub origin_session_id: Option<Uuid>,
    #[ts(type = "number")]
    pub hop_count: i64,
    pub comment_author_id: Option<Uuid>,
    pub comment_created_at: DateTime<Utc>,
    /// Exact text sent to the agent (snapshot; later edits are not replayed).
    pub delivered_message: Option<String>,
    /// The comment's `updated_at` when it was delivered; a newer `updated_at`
    /// means "edited after delivery".
    pub delivered_comment_updated_at: Option<DateTime<Utc>>,
    #[ts(type = "number")]
    pub attempt_count: i64,
    pub next_attempt_at: Option<DateTime<Utc>>,
    /// An explicit target choice, which may resume a stopped turn.
    pub allow_stopped_target: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
}

/// Inputs for a new delivery row.
#[derive(Debug, Clone)]
pub struct NewIssueCommentDelivery {
    pub comment_id: Uuid,
    pub issue_id: Uuid,
    pub source: IssueCommentDeliverySource,
    pub origin_session_id: Option<Uuid>,
    pub comment_author_id: Option<Uuid>,
    pub comment_created_at: DateTime<Utc>,
}

/// The outcome of target resolution, written in one update.
#[derive(Debug, Clone, PartialEq)]
pub struct IssueCommentDeliveryResolution {
    pub state: IssueCommentDeliveryState,
    pub reason: Option<IssueCommentDeliveryReason>,
    pub error: Option<String>,
    pub workspace_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub hop_count: i64,
    pub allow_stopped_target: bool,
}

const SELECT: &str = "SELECT id, comment_id, issue_id, workspace_id, session_id, \
    execution_process_id, state, reason, error, source, origin_session_id, hop_count, \
    comment_author_id, comment_created_at, delivered_message, delivered_comment_updated_at, \
    attempt_count, next_attempt_at, allow_stopped_target, created_at, updated_at, delivered_at \
    FROM issue_comment_deliveries";

impl IssueCommentDelivery {
    /// Insert a row for `comment_id`, already resolved, unless one exists.
    /// Returns the stored row and whether this call created it. Resolving
    /// before inserting means no row ever waits without a target: a crash
    /// mid-resolution leaves no row at all, so a retry starts cleanly.
    pub async fn insert_if_absent(
        pool: &SqlitePool,
        new: &NewIssueCommentDelivery,
        resolution: &IssueCommentDeliveryResolution,
        now: DateTime<Utc>,
    ) -> Result<(Self, bool), sqlx::Error> {
        let result = sqlx::query(
            "INSERT INTO issue_comment_deliveries (id, comment_id, issue_id, state, reason, \
             error, workspace_id, session_id, hop_count, allow_stopped_target, source, \
             origin_session_id, comment_author_id, comment_created_at, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(comment_id) DO NOTHING",
        )
        .bind(Uuid::new_v4())
        .bind(new.comment_id)
        .bind(new.issue_id)
        .bind(resolution.state)
        .bind(resolution.reason)
        .bind(&resolution.error)
        .bind(resolution.workspace_id)
        .bind(resolution.session_id)
        .bind(resolution.hop_count)
        .bind(resolution.allow_stopped_target)
        .bind(new.source)
        .bind(new.origin_session_id)
        .bind(new.comment_author_id)
        .bind(new.comment_created_at)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;
        let row = Self::find_by_comment_id(pool, new.comment_id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;
        Ok((row, result.rows_affected() == 1))
    }

    /// Persist the exact text about to be sent, before the turn starts, so
    /// recovery after a crash keeps what the agent actually received.
    pub async fn record_snapshot(
        pool: &SqlitePool,
        id: Uuid,
        message: &str,
        comment_updated_at: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE issue_comment_deliveries SET delivered_message = ?, \
             delivered_comment_updated_at = ?, updated_at = ? \
             WHERE id = ? AND state = 'dispatching'",
        )
        .bind(message)
        .bind(comment_updated_at)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(&format!("{SELECT} WHERE id = ?"))
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    pub async fn find_by_comment_id(
        pool: &SqlitePool,
        comment_id: Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(&format!("{SELECT} WHERE comment_id = ?"))
            .bind(comment_id)
            .fetch_optional(pool)
            .await
    }

    pub async fn list_by_issue(
        pool: &SqlitePool,
        issue_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(&format!(
            "{SELECT} WHERE issue_id = ? ORDER BY comment_created_at ASC"
        ))
        .bind(issue_id)
        .fetch_all(pool)
        .await
    }

    /// The delivery that started `execution_process_id`, if any (loop guard).
    pub async fn find_by_execution_process_id(
        pool: &SqlitePool,
        execution_process_id: Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(&format!(
            "{SELECT} WHERE execution_process_id = ? ORDER BY hop_count DESC LIMIT 1"
        ))
        .bind(execution_process_id)
        .fetch_optional(pool)
        .await
    }

    /// Pending rows for one session, oldest comment first.
    pub async fn list_pending_for_session(
        pool: &SqlitePool,
        session_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(&format!(
            "{SELECT} WHERE session_id = ? AND state = 'pending' ORDER BY comment_created_at ASC"
        ))
        .bind(session_id)
        .fetch_all(pool)
        .await
    }

    /// Sessions that have pending rows whose retry time (if any) has come.
    pub async fn sessions_with_due_pending(
        pool: &SqlitePool,
        now: DateTime<Utc>,
    ) -> Result<Vec<Uuid>, sqlx::Error> {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT DISTINCT session_id FROM issue_comment_deliveries \
             WHERE state = 'pending' AND session_id IS NOT NULL \
             AND (next_attempt_at IS NULL OR next_attempt_at <= ?)",
        )
        .bind(now)
        .fetch_all(pool)
        .await
    }

    pub async fn list_dispatching(pool: &SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(&format!("{SELECT} WHERE state = 'dispatching'"))
            .fetch_all(pool)
            .await
    }

    /// Write a resolution, only if the row is still in `from`. A manual
    /// re-resolution also resets the automatic attempt budget.
    pub async fn apply_resolution(
        pool: &SqlitePool,
        id: Uuid,
        from: &[IssueCommentDeliveryState],
        resolution: &IssueCommentDeliveryResolution,
        now: DateTime<Utc>,
    ) -> Result<bool, sqlx::Error> {
        let placeholders = vec!["?"; from.len()].join(", ");
        let sql = format!(
            "UPDATE issue_comment_deliveries SET state = ?, reason = ?, error = ?, \
             workspace_id = ?, session_id = ?, hop_count = ?, allow_stopped_target = ?, \
             attempt_count = 0, next_attempt_at = NULL, updated_at = ? \
             WHERE id = ? AND state IN ({placeholders})"
        );
        let mut query = sqlx::query(&sql)
            .bind(resolution.state)
            .bind(resolution.reason)
            .bind(&resolution.error)
            .bind(resolution.workspace_id)
            .bind(resolution.session_id)
            .bind(resolution.hop_count)
            .bind(resolution.allow_stopped_target)
            .bind(now)
            .bind(id);
        for state in from {
            query = query.bind(state.as_str());
        }
        Ok(query.execute(pool).await?.rows_affected() == 1)
    }

    /// Take ownership of a pending row. Exactly one caller wins.
    pub async fn claim(
        pool: &SqlitePool,
        id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE issue_comment_deliveries SET state = 'dispatching', \
             attempt_count = attempt_count + 1, updated_at = ? \
             WHERE id = ? AND state = 'pending'",
        )
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    /// Record the started execution and the exact text it carried.
    pub async fn mark_delivered(
        pool: &SqlitePool,
        id: Uuid,
        execution_process_id: Uuid,
        delivered_message: &str,
        comment_updated_at: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE issue_comment_deliveries SET state = 'delivered', reason = NULL, error = NULL, \
             execution_process_id = ?, delivered_message = ?, delivered_comment_updated_at = ?, \
             next_attempt_at = NULL, delivered_at = ?, updated_at = ? \
             WHERE id = ? AND state = 'dispatching'",
        )
        .bind(execution_process_id)
        .bind(delivered_message)
        .bind(comment_updated_at)
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    /// Move a claimed row to a non-delivered outcome. `next_attempt_at` is
    /// only meaningful for `Pending` (a scheduled retry).
    pub async fn finish_dispatch(
        pool: &SqlitePool,
        id: Uuid,
        state: IssueCommentDeliveryState,
        reason: Option<IssueCommentDeliveryReason>,
        error: Option<&str>,
        next_attempt_at: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE issue_comment_deliveries SET state = ?, reason = ?, error = ?, \
             next_attempt_at = ?, updated_at = ? WHERE id = ? AND state = 'dispatching'",
        )
        .bind(state)
        .bind(reason)
        .bind(error)
        .bind(next_attempt_at)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    /// Update the reason a pending row is still waiting (no state change).
    pub async fn set_pending_reason(
        pool: &SqlitePool,
        id: Uuid,
        reason: IssueCommentDeliveryReason,
        now: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE issue_comment_deliveries SET reason = ?, updated_at = ? \
             WHERE id = ? AND state = 'pending'",
        )
        .bind(reason)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Move a pending row straight to an outcome (e.g. its target's turn
    /// stopped while it waited). No-op if it is no longer pending.
    pub async fn finish_pending(
        pool: &SqlitePool,
        id: Uuid,
        state: IssueCommentDeliveryState,
        reason: IssueCommentDeliveryReason,
        now: DateTime<Utc>,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE issue_comment_deliveries SET state = ?, reason = ?, updated_at = ? \
             WHERE id = ? AND state = 'pending'",
        )
        .bind(state)
        .bind(reason)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;

    async fn test_pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    fn new_delivery(comment_id: Uuid) -> NewIssueCommentDelivery {
        NewIssueCommentDelivery {
            comment_id,
            issue_id: Uuid::new_v4(),
            source: IssueCommentDeliverySource::Ui,
            origin_session_id: None,
            comment_author_id: Some(Uuid::new_v4()),
            comment_created_at: Utc::now(),
        }
    }

    fn pending_resolution() -> IssueCommentDeliveryResolution {
        IssueCommentDeliveryResolution {
            state: IssueCommentDeliveryState::Pending,
            reason: None,
            error: None,
            workspace_id: None,
            session_id: None,
            hop_count: 0,
            allow_stopped_target: false,
        }
    }

    #[tokio::test]
    async fn duplicate_requests_share_one_row() {
        let pool = test_pool().await;
        let comment_id = Uuid::new_v4();
        let (first, created) = IssueCommentDelivery::insert_if_absent(
            &pool,
            &new_delivery(comment_id),
            &pending_resolution(),
            Utc::now(),
        )
        .await
        .unwrap();
        assert!(created);
        let (second, created_again) = IssueCommentDelivery::insert_if_absent(
            &pool,
            &new_delivery(comment_id),
            &pending_resolution(),
            Utc::now(),
        )
        .await
        .unwrap();
        assert!(!created_again);
        assert_eq!(first.id, second.id);
        // The first request's attribution is kept.
        assert_eq!(first.issue_id, second.issue_id);
    }

    #[tokio::test]
    async fn only_one_claim_wins_and_only_the_claimant_finishes() {
        let pool = test_pool().await;
        let (row, _) = IssueCommentDelivery::insert_if_absent(
            &pool,
            &new_delivery(Uuid::new_v4()),
            &pending_resolution(),
            Utc::now(),
        )
        .await
        .unwrap();
        assert!(
            IssueCommentDelivery::claim(&pool, row.id, Utc::now())
                .await
                .unwrap()
        );
        assert!(
            !IssueCommentDelivery::claim(&pool, row.id, Utc::now())
                .await
                .unwrap()
        );

        // A delivered row cannot be re-claimed or re-finished.
        let execution = Uuid::new_v4();
        // No such execution row exists; disable FK enforcement for this
        // state-machine check only.
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            IssueCommentDelivery::mark_delivered(&pool, row.id, execution, "hi", None, Utc::now())
                .await
                .unwrap()
        );
        assert!(
            !IssueCommentDelivery::finish_dispatch(
                &pool,
                row.id,
                IssueCommentDeliveryState::Failed,
                Some(IssueCommentDeliveryReason::StartFailed),
                Some("late"),
                None,
                Utc::now(),
            )
            .await
            .unwrap()
        );
        let stored = IssueCommentDelivery::find_by_id(&pool, row.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.state, IssueCommentDeliveryState::Delivered);
        assert_eq!(stored.execution_process_id, Some(execution));
        assert_eq!(stored.attempt_count, 1);
        assert_eq!(
            IssueCommentDelivery::find_by_execution_process_id(&pool, execution)
                .await
                .unwrap()
                .map(|d| d.id),
            Some(row.id)
        );
    }

    #[tokio::test]
    async fn resolution_applies_only_from_allowed_states_and_resets_attempts() {
        let pool = test_pool().await;
        let (row, _) = IssueCommentDelivery::insert_if_absent(
            &pool,
            &new_delivery(Uuid::new_v4()),
            &pending_resolution(),
            Utc::now(),
        )
        .await
        .unwrap();
        IssueCommentDelivery::claim(&pool, row.id, Utc::now())
            .await
            .unwrap();
        IssueCommentDelivery::finish_dispatch(
            &pool,
            row.id,
            IssueCommentDeliveryState::Failed,
            Some(IssueCommentDeliveryReason::StartFailed),
            Some("boom"),
            None,
            Utc::now(),
        )
        .await
        .unwrap();

        // Not resolvable from Delivered-only filter.
        assert!(
            !IssueCommentDelivery::apply_resolution(
                &pool,
                row.id,
                &[IssueCommentDeliveryState::Delivered],
                &pending_resolution(),
                Utc::now(),
            )
            .await
            .unwrap()
        );
        assert!(
            IssueCommentDelivery::apply_resolution(
                &pool,
                row.id,
                &[IssueCommentDeliveryState::Failed],
                &pending_resolution(),
                Utc::now(),
            )
            .await
            .unwrap()
        );
        let stored = IssueCommentDelivery::find_by_id(&pool, row.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.state, IssueCommentDeliveryState::Pending);
        assert_eq!(stored.attempt_count, 0);
        assert_eq!(stored.error, None);
    }

    #[tokio::test]
    async fn due_sessions_respect_retry_time() {
        let pool = test_pool().await;
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&pool)
            .await
            .unwrap();
        let session = Uuid::new_v4();
        let (row, _) = IssueCommentDelivery::insert_if_absent(
            &pool,
            &new_delivery(Uuid::new_v4()),
            &pending_resolution(),
            Utc::now(),
        )
        .await
        .unwrap();
        let mut resolution = pending_resolution();
        resolution.session_id = Some(session);
        IssueCommentDelivery::apply_resolution(
            &pool,
            row.id,
            &[IssueCommentDeliveryState::Pending],
            &resolution,
            Utc::now(),
        )
        .await
        .unwrap();
        assert_eq!(
            IssueCommentDelivery::sessions_with_due_pending(&pool, Utc::now())
                .await
                .unwrap(),
            vec![session]
        );

        IssueCommentDelivery::claim(&pool, row.id, Utc::now())
            .await
            .unwrap();
        let retry_at = Utc::now() + Duration::seconds(60);
        IssueCommentDelivery::finish_dispatch(
            &pool,
            row.id,
            IssueCommentDeliveryState::Pending,
            Some(IssueCommentDeliveryReason::RetryScheduled),
            Some("transient"),
            Some(retry_at),
            Utc::now(),
        )
        .await
        .unwrap();
        assert!(
            IssueCommentDelivery::sessions_with_due_pending(&pool, Utc::now())
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            IssueCommentDelivery::sessions_with_due_pending(&pool, retry_at + Duration::seconds(1))
                .await
                .unwrap(),
            vec![session]
        );
    }
}
