use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool, Type};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, TS)]
#[sqlx(type_name = "browser_session_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[ts(use_ts_enum)]
pub enum BrowserSessionDbStatus {
    Starting,
    Running,
    Closed,
    Failed,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
pub struct BrowserSession {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub host_id: String,
    pub profile: Option<String>,
    pub status: BrowserSessionDbStatus,
    pub current_url: Option<String>,
    /// OS process group id of the spawned Chromium process group, used to
    /// clean up orphaned process groups after a server crash. Not
    /// meaningful across machines.
    pub pgid: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct CreateBrowserSession {
    pub workspace_id: Uuid,
    pub profile: Option<String>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
pub struct BrowserControlTransition {
    pub id: Uuid,
    pub browser_session_id: Uuid,
    pub generation: i64,
    pub controller_type: String,
    pub execution_id: Option<Uuid>,
    pub user_id: Option<String>,
    pub connection_id: Option<Uuid>,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}

const SESSION_COLUMNS: &str = r#"id as "id!: Uuid", workspace_id as "workspace_id!: Uuid", host_id, profile, status as "status!: BrowserSessionDbStatus", current_url, pgid, created_at as "created_at!: DateTime<Utc>", updated_at as "updated_at!: DateTime<Utc>", closed_at as "closed_at: DateTime<Utc>", expires_at as "expires_at: DateTime<Utc>""#;
// SESSION_COLUMNS is documentation for the repeated projections below;
// sqlx macros need literal strings, so each query repeats it verbatim.
const _: &str = SESSION_COLUMNS;

impl BrowserSession {
    pub async fn create(
        pool: &SqlitePool,
        id: Uuid,
        workspace_id: Uuid,
        host_id: &str,
        profile: Option<&str>,
        status: BrowserSessionDbStatus,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            BrowserSession,
            r#"INSERT INTO browser_sessions (id, workspace_id, host_id, profile, status, expires_at)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id as "id!: Uuid", workspace_id as "workspace_id!: Uuid", host_id, profile, status as "status!: BrowserSessionDbStatus", current_url, pgid, created_at as "created_at!: DateTime<Utc>", updated_at as "updated_at!: DateTime<Utc>", closed_at as "closed_at: DateTime<Utc>", expires_at as "expires_at: DateTime<Utc>""#,
            id,
            workspace_id,
            host_id,
            profile,
            status,
            expires_at
        )
        .fetch_one(pool)
        .await
    }

    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            BrowserSession,
            r#"SELECT id as "id!: Uuid", workspace_id as "workspace_id!: Uuid", host_id, profile, status as "status!: BrowserSessionDbStatus", current_url, pgid, created_at as "created_at!: DateTime<Utc>", updated_at as "updated_at!: DateTime<Utc>", closed_at as "closed_at: DateTime<Utc>", expires_at as "expires_at: DateTime<Utc>"
               FROM browser_sessions WHERE id = $1"#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_rowid(pool: &SqlitePool, rowid: i64) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            BrowserSession,
            r#"SELECT id as "id!: Uuid", workspace_id as "workspace_id!: Uuid", host_id, profile, status as "status!: BrowserSessionDbStatus", current_url, pgid, created_at as "created_at!: DateTime<Utc>", updated_at as "updated_at!: DateTime<Utc>", closed_at as "closed_at: DateTime<Utc>", expires_at as "expires_at: DateTime<Utc>"
               FROM browser_sessions WHERE rowid = $1"#,
            rowid
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_workspace(
        pool: &SqlitePool,
        workspace_id: Uuid,
        include_closed: bool,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            BrowserSession,
            r#"SELECT id as "id!: Uuid", workspace_id as "workspace_id!: Uuid", host_id, profile, status as "status!: BrowserSessionDbStatus", current_url, pgid, created_at as "created_at!: DateTime<Utc>", updated_at as "updated_at!: DateTime<Utc>", closed_at as "closed_at: DateTime<Utc>", expires_at as "expires_at: DateTime<Utc>"
               FROM browser_sessions
               WHERE workspace_id = $1 AND ($2 OR status NOT IN ('closed', 'failed'))
               ORDER BY created_at DESC"#,
            workspace_id,
            include_closed
        )
        .fetch_all(pool)
        .await
    }

    pub async fn find_open(pool: &SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            BrowserSession,
            r#"SELECT id as "id!: Uuid", workspace_id as "workspace_id!: Uuid", host_id, profile, status as "status!: BrowserSessionDbStatus", current_url, pgid, created_at as "created_at!: DateTime<Utc>", updated_at as "updated_at!: DateTime<Utc>", closed_at as "closed_at: DateTime<Utc>", expires_at as "expires_at: DateTime<Utc>"
               FROM browser_sessions
               WHERE status IN ('starting', 'running')"#
        )
        .fetch_all(pool)
        .await
    }

    pub async fn update_status(
        pool: &SqlitePool,
        id: Uuid,
        status: BrowserSessionDbStatus,
    ) -> Result<(), sqlx::Error> {
        let closed = matches!(
            status,
            BrowserSessionDbStatus::Closed | BrowserSessionDbStatus::Failed
        );
        sqlx::query!(
            r#"UPDATE browser_sessions
               SET status = $2,
                   closed_at = CASE WHEN $3 THEN datetime('now', 'subsec') ELSE closed_at END,
                   updated_at = datetime('now', 'subsec')
               WHERE id = $1"#,
            id,
            status,
            closed
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn update_activity(
        pool: &SqlitePool,
        id: Uuid,
        current_url: Option<&str>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE browser_sessions
               SET current_url = $2, expires_at = $3, updated_at = datetime('now', 'subsec')
               WHERE id = $1"#,
            id,
            current_url,
            expires_at
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn update_pgid(pool: &SqlitePool, id: Uuid, pgid: i64) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE browser_sessions SET pgid = $1 WHERE id = $2"#,
            pgid,
            id
        )
        .execute(pool)
        .await?;
        Ok(())
    }
}

impl BrowserControlTransition {
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        pool: &SqlitePool,
        browser_session_id: Uuid,
        generation: i64,
        controller_type: &str,
        execution_id: Option<Uuid>,
        user_id: Option<&str>,
        connection_id: Option<Uuid>,
        reason: &str,
    ) -> Result<(), sqlx::Error> {
        let id = Uuid::new_v4();
        sqlx::query!(
            r#"INSERT INTO browser_control_transitions
               (id, browser_session_id, generation, controller_type, execution_id, user_id, connection_id, reason)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
            id,
            browser_session_id,
            generation,
            controller_type,
            execution_id,
            user_id,
            connection_id,
            reason
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn find_by_session(
        pool: &SqlitePool,
        browser_session_id: Uuid,
        limit: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            BrowserControlTransition,
            r#"SELECT id as "id!: Uuid", browser_session_id as "browser_session_id!: Uuid", generation, controller_type, execution_id as "execution_id: Uuid", user_id, connection_id as "connection_id: Uuid", reason, created_at as "created_at!: DateTime<Utc>"
               FROM browser_control_transitions
               WHERE browser_session_id = $1
               ORDER BY created_at DESC
               LIMIT $2"#,
            browser_session_id,
            limit
        )
        .fetch_all(pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::{BrowserSession, BrowserSessionDbStatus};
    use crate::models::workspace::{CreateWorkspace, Workspace};

    async fn test_pool() -> sqlx::SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    async fn seed_workspace(pool: &sqlx::SqlitePool) -> Uuid {
        let id = Uuid::new_v4();
        Workspace::create(
            pool,
            &CreateWorkspace {
                branch: "vk/browser-session-pgid-test".to_string(),
                name: None,
            },
            id,
        )
        .await
        .unwrap();
        id
    }

    // The startup reaper depends on: a persisted pgid, `find_open` surfacing it
    // on non-closed rows, and closed rows dropping out of that set.
    #[tokio::test]
    async fn pgid_round_trips_and_find_open_tracks_status() {
        let pool = test_pool().await;
        let workspace_id = seed_workspace(&pool).await;
        let id = Uuid::new_v4();

        let row = BrowserSession::create(
            &pool,
            id,
            workspace_id,
            "host-a",
            None,
            BrowserSessionDbStatus::Running,
            None,
        )
        .await
        .unwrap();
        // A freshly created session has no pgid until the group is spawned.
        assert_eq!(row.pgid, None);

        BrowserSession::update_pgid(&pool, id, 4242).await.unwrap();
        let stored = BrowserSession::find_by_id(&pool, id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.pgid, Some(4242));

        // The reaper walks `find_open`; the row is visible with its pgid while
        // running.
        let open = BrowserSession::find_open(&pool).await.unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, id);
        assert_eq!(open[0].pgid, Some(4242));

        // Once closed it drops out of the reaper's candidate set.
        BrowserSession::update_status(&pool, id, BrowserSessionDbStatus::Closed)
            .await
            .unwrap();
        assert!(BrowserSession::find_open(&pool).await.unwrap().is_empty());
    }
}
