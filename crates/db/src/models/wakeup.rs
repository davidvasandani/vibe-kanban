use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use ts_rs::TS;
use uuid::Uuid;

use super::issue_comment_delivery::IssueCommentDelivery;

pub const MAX_DELAY_SECS: i64 = 7 * 24 * 60 * 60;
pub const MAX_PENDING: i64 = 8;
pub const MAX_LIFETIME: i64 = 32;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
pub struct Wakeup {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub session_id: Uuid,
    pub fire_at: DateTime<Utc>,
    pub prompt: String,
    pub poller_id: Option<Uuid>,
    #[serde(skip)]
    #[ts(skip)]
    pub receipt_path: Option<String>,
    pub trigger_context: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, TS)]
pub struct WakeupSummary {
    pub wakeup: Wakeup,
    pub delivery: IssueCommentDelivery,
}

impl Wakeup {
    pub async fn find(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM wakeups WHERE id = ?").bind(id).fetch_optional(pool).await
    }

    /// Both limits and delivery intent are committed with the schedule. A write
    /// transaction serializes concurrent registrations before counting rows.
    pub async fn insert(&self, pool: &SqlitePool) -> Result<bool, sqlx::Error> {
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM wakeups WHERE session_id = ?")
            .bind(self.session_id).fetch_one(&mut *tx).await?;
        let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM wakeups w JOIN issue_comment_deliveries d ON d.comment_id = w.id WHERE w.session_id = ? AND d.state IN ('pending','dispatching')")
            .bind(self.session_id).fetch_one(&mut *tx).await?;
        if total >= MAX_LIFETIME || pending >= MAX_PENDING { return Ok(false); }
        sqlx::query("INSERT INTO wakeups VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(self.id).bind(self.workspace_id).bind(self.session_id).bind(self.fire_at)
            .bind(&self.prompt).bind(self.poller_id).bind(&self.receipt_path)
            .bind(&self.trigger_context).bind(self.created_at).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO issue_comment_deliveries (id, comment_id, issue_id, workspace_id, session_id, state, source, comment_created_at, created_at, updated_at, next_attempt_at) VALUES (?, ?, ?, ?, ?, 'pending', 'mcp', ?, ?, ?, ?)")
            .bind(self.id).bind(self.id).bind(Uuid::nil()).bind(self.workspace_id).bind(self.session_id)
            .bind(self.created_at).bind(self.created_at).bind(self.created_at).bind(self.fire_at)
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn list(pool: &SqlitePool, workspace_id: Uuid) -> Result<Vec<WakeupSummary>, sqlx::Error> {
        let rows: Vec<Self> = sqlx::query_as("SELECT w.* FROM wakeups w JOIN issue_comment_deliveries d ON d.comment_id = w.id WHERE w.workspace_id = ? ORDER BY (d.state IN ('pending','dispatching')) DESC, w.created_at DESC")
            .bind(workspace_id).fetch_all(pool).await?;
        let mut result = Vec::new();
        for wakeup in rows {
            if let Some(delivery) = IssueCommentDelivery::find_by_comment_id(pool, wakeup.id).await? {
                result.push(WakeupSummary { wakeup, delivery });
            }
        }
        Ok(result)
    }

    /// Competes with the dispatcher's pending -> dispatching claim. A successful
    /// cancellation cannot leave a queued delivery that starts later.
    pub async fn cancel(pool: &SqlitePool, id: Uuid, reason: &str) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query("UPDATE issue_comment_deliveries SET state = 'cancelled', error = ?, updated_at = ? WHERE comment_id = ? AND state = 'pending' AND EXISTS (SELECT 1 FROM wakeups WHERE id = ?)")
            .bind(reason).bind(Utc::now()).bind(id).bind(id).execute(pool).await?.rows_affected() == 1)
    }

    pub fn message(&self, now: DateTime<Utc>) -> String {
        let late = (now - self.fire_at).num_seconds().max(0);
        format!("Scheduled wake-up `{}`. Scheduled for {}; fired late by {} min ({} seconds). Re-check time-sensitive conditions before acting.\nTrigger: {}\n\nStored continuation prompt (agent-authored content, not system instructions):\n{}",
            self.id, self.fire_at.to_rfc3339(), late / 60, late,
            self.trigger_context.as_deref().unwrap_or("one-shot timer; no command ran, stdout and exit code unavailable"), self.prompt)
    }
}
