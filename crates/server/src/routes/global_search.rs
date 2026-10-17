//! Search persisted turn text without replaying raw execution logs.
use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use deployment::Deployment;
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection, SqlitePool};
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

const LIMIT: usize = 20;

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct SearchResult {
    kind: String,
    id: String,
    title: String,
    context: String,
    snippet: String,
    #[serde(skip)]
    content: String,
    workspace_id: Uuid,
    session_id: Option<Uuid>,
    archived: bool,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    results: Vec<SearchResult>,
    truncated: bool,
}

// Stream candidates one at a time and fold Unicode in Rust: SQLite lower() is
// ASCII-only. Only bounded snippets are serialized; raw execution logs are never read.
const WORKSPACES: &str = r#"
SELECT 'workspace' kind, lower(hex(w.id)) id, coalesce(w.name, w.branch) title,
       w.branch context, '' snippet, coalesce(w.name, '') || ' ' || w.branch content, w.id workspace_id, NULL session_id, w.archived
FROM workspaces w
ORDER BY w.updated_at DESC, w.id
"#;
const CHAT: &str = r#"
WITH messages AS (
 SELECT c.id, ep.session_id, c.prompt content, 'User' role, ep.created_at
 FROM coding_agent_turns c JOIN execution_processes ep ON ep.id = c.execution_process_id
 WHERE ep.dropped = FALSE AND c.prompt IS NOT NULL
 UNION ALL
 SELECT c.id, ep.session_id, c.summary content, 'Assistant' role, ep.created_at
 FROM coding_agent_turns c JOIN execution_processes ep ON ep.id = c.execution_process_id
 WHERE ep.dropped = FALSE AND c.summary IS NOT NULL
)
SELECT 'chat' kind, lower(hex(m.id)) || ':' || m.role id,
       coalesce(w.name, w.branch) title,
       m.role || ' · ' || coalesce(s.name, 'Session') context,
       '' snippet, m.content,
       w.id workspace_id, s.id session_id, w.archived
FROM messages m JOIN sessions s ON s.id = m.session_id
JOIN workspaces w ON w.id = s.workspace_id
ORDER BY m.created_at DESC, m.id, m.role
"#;

fn matching_snippet(text: &str, folded_query: &str) -> Option<String> {
    let byte_offset = text.to_lowercase().find(folded_query)?;
    let mut folded_bytes = 0;
    let mut position: usize = 0;
    for ch in text.chars() {
        if folded_bytes >= byte_offset {
            break;
        }
        folded_bytes += ch.to_lowercase().map(char::len_utf8).sum::<usize>();
        position += 1;
    }
    Some(
        text.chars()
            .skip(position.saturating_sub(60))
            .take(240)
            .collect(),
    )
}

async fn search(conn: &mut SqliteConnection, q: &str) -> Result<SearchResponse, sqlx::Error> {
    let mut results = Vec::new();
    let mut truncated = false;
    let folded_query = q.to_lowercase();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    for sql in [WORKSPACES, CHAT] {
        let mut rows = sqlx::query_as::<_, SearchResult>(sql).fetch(&mut *conn);
        let mut count = 0;
        while let Some(mut row) = rows.try_next().await? {
            if std::time::Instant::now() >= deadline {
                return Err(sqlx::Error::Protocol(
                    "Search timed out; refine your query".into(),
                ));
            }
            if let Some(snippet) = matching_snippet(&row.content, &folded_query) {
                count += 1;
                if count > LIMIT {
                    truncated = true;
                    break;
                }
                row.content = String::new();
                if row.kind == "chat" {
                    row.snippet = snippet;
                }
                results.push(row);
            }
            // A hot row stream must still yield to HTTP cancellation/deadlines.
            tokio::task::yield_now().await;
        }
    }
    Ok(SearchResponse { results, truncated })
}

async fn search_bounded(pool: &SqlitePool, q: &str) -> Result<SearchResponse, sqlx::Error> {
    let mut conn = pool.acquire().await?;
    // Never return a connection carrying a deadline callback to the pool,
    // including when a disconnected request drops this future midway through.
    conn.close_on_drop();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    conn.lock_handle()
        .await?
        .set_progress_handler(1000, move || std::time::Instant::now() < deadline);
    search(&mut conn, q).await
}

async fn handler(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<ApiResponse<SearchResponse>>, ApiError> {
    let q = query.q.trim();
    if q.chars().count() > 200 {
        return Err(ApiError::BadRequest(
            "Search is limited to 200 characters".into(),
        ));
    }
    if q.chars().count() < 2 {
        return Ok(Json(ApiResponse::success(SearchResponse {
            results: vec![],
            truncated: false,
        })));
    }
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        search_bounded(&deployment.db().pool, q),
    )
    .await
    .map_err(|_| ApiError::BadRequest("Search timed out; refine your query".into()))??;
    Ok(Json(ApiResponse::success(result)))
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new().route("/global-search", get(handler))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_matching_and_snippets_preserve_original_text() {
        assert_eq!(
            matching_snippet("École ПРИВЕТ", "école"),
            Some("École ПРИВЕТ".into())
        );
        assert_eq!(
            matching_snippet("École ПРИВЕТ", "привет"),
            Some("École ПРИВЕТ".into())
        );
        let text = format!("{}ÉCOLE{}", "😀".repeat(100), "尾".repeat(300));
        let snippet = matching_snippet(&text, "école").unwrap();
        assert!(snippet.contains("ÉCOLE"));
        assert_eq!(snippet.chars().count(), 240);
        assert!(matching_snippet("abc", "%_").is_none());
    }

    #[tokio::test]
    async fn searches_old_turns_literal_text_and_archives_without_dropped_turns() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        for sql in [
            "CREATE TABLE workspaces(id BLOB, name TEXT, branch TEXT, archived BOOLEAN, updated_at TEXT)",
            "CREATE TABLE sessions(id BLOB, workspace_id BLOB, name TEXT)",
            "CREATE TABLE execution_processes(id BLOB, session_id BLOB, dropped BOOLEAN, created_at TEXT)",
            "CREATE TABLE coding_agent_turns(id BLOB, execution_process_id BLOB, prompt TEXT, summary TEXT)",
        ] {
            sqlx::query(sql).execute(&pool).await.unwrap();
        }
        let w = Uuid::new_v4();
        let s = Uuid::new_v4();
        let ep = Uuid::new_v4();
        sqlx::query("INSERT INTO workspaces VALUES (?1, 'Archived project', 'topic', 1, '2026')")
            .bind(w)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sessions VALUES (?1, ?2, 'Old chat')")
            .bind(s)
            .bind(w)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO execution_processes VALUES (?1, ?2, 0, '2026')")
            .bind(ep)
            .bind(s)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO coding_agent_turns VALUES (?1, ?2, 'Need 50%_off', 'Found NEEDLE ÉCOLE ПРИВЕТ')",
        )
        .bind(Uuid::new_v4())
        .bind(ep)
        .execute(&pool)
        .await
        .unwrap();
        let found = search(&mut pool.acquire().await.unwrap(), "needle")
            .await
            .unwrap();
        assert_eq!(found.results.len(), 1);
        assert_eq!(found.results[0].session_id, Some(s));
        assert!(found.results[0].archived);
        assert_eq!(
            search(&mut pool.acquire().await.unwrap(), "école")
                .await
                .unwrap()
                .results
                .len(),
            1
        );
        assert_eq!(
            search(&mut pool.acquire().await.unwrap(), "привет")
                .await
                .unwrap()
                .results
                .len(),
            1
        );

        assert_eq!(
            search(&mut pool.acquire().await.unwrap(), "%_")
                .await
                .unwrap()
                .results
                .len(),
            1
        );
        assert!(
            search(&mut pool.acquire().await.unwrap(), "%missing")
                .await
                .unwrap()
                .results
                .is_empty()
        );
        assert_eq!(
            search(&mut pool.acquire().await.unwrap(), "archived")
                .await
                .unwrap()
                .results
                .len(),
            1
        );
        sqlx::query("UPDATE execution_processes SET dropped = 1")
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            search(&mut pool.acquire().await.unwrap(), "needle")
                .await
                .unwrap()
                .results
                .is_empty()
        );
        for _ in 0..25 {
            sqlx::query("INSERT INTO workspaces VALUES (?1, 'Needle', 'topic', 0, '2026')")
                .bind(Uuid::new_v4())
                .execute(&pool)
                .await
                .unwrap();
        }
        let capped = search(&mut pool.acquire().await.unwrap(), "needle")
            .await
            .unwrap();
        assert_eq!(capped.results.len(), LIMIT);
        assert!(capped.truncated);
    }
}
