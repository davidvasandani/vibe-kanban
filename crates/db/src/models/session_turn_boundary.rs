//! Durable record of a session's last finalized turn boundary.

use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use super::execution_process::ExecutionProcess;

pub struct SessionTurnBoundary;

impl SessionTurnBoundary {
    /// Record that the session's current latest coding-agent turn reached its
    /// boundary. Called from `finalize_task`, where the exit monitor has
    /// decided not to start a queued follow-up, so the latest turn here is
    /// exactly the chain that just ended.
    pub async fn record(pool: &SqlitePool, session_id: Uuid) -> Result<(), sqlx::Error> {
        let Some((turn_id, _, _)) =
            ExecutionProcess::latest_coding_agent_turn_for_session(pool, session_id).await?
        else {
            return Ok(());
        };
        sqlx::query(
            "INSERT INTO session_turn_boundaries (session_id, turn_execution_id, finalized_at) \
             VALUES (?, ?, ?) ON CONFLICT(session_id) DO UPDATE SET \
             turn_execution_id = excluded.turn_execution_id, finalized_at = excluded.finalized_at",
        )
        .bind(session_id)
        .bind(turn_id)
        .bind(Utc::now())
        .execute(pool)
        .await?;
        Ok(())
    }

    /// The turn whose boundary was last recorded for the session.
    pub async fn finalized_turn(
        pool: &SqlitePool,
        session_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT turn_execution_id FROM session_turn_boundaries WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_optional(pool)
        .await
    }
}
