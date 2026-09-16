//! Membership-scoped metadata search; chat remains on authorized local hosts.
use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::error::{ErrorResponse, db_error};
use crate::{AppState, auth::RequestContext};

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}
#[derive(Serialize, FromRow)]
struct SearchResult {
    kind: String,
    id: Uuid,
    title: String,
    context: String,
    snippet: String,
    organization_id: Uuid,
    project_id: Option<Uuid>,
    workspace_id: Option<Uuid>,
    issue_id: Option<Uuid>,
    archived: bool,
}
#[derive(Serialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
    truncated: bool,
}

const SEARCH: &str = r#"
WITH accessible AS (
 SELECT o.id, o.name FROM organizations o
 JOIN organization_member_metadata m ON m.organization_id = o.id
 WHERE m.user_id = $1
), matches AS (
 SELECT 'organization'::text kind, o.id, o.name title, 'Organization'::text context,
 ''::text snippet, o.id organization_id, NULL::uuid project_id,
 NULL::uuid workspace_id, NULL::uuid issue_id, false archived
 FROM accessible o WHERE strpos(lower(o.name), lower($2)) > 0
 UNION ALL
 SELECT 'project', p.id, p.name, o.name, '', o.id, p.id, NULL, NULL, false
 FROM projects p JOIN accessible o ON o.id = p.organization_id
 WHERE strpos(lower(p.name), lower($2)) > 0
 UNION ALL
 SELECT 'workspace', w.id, coalesce(w.name, 'Workspace'), o.name || ' / ' || p.name,
 '', o.id, p.id, w.local_workspace_id, w.issue_id, w.archived
 FROM workspaces w JOIN projects p ON p.id = w.project_id
 JOIN accessible o ON o.id = p.organization_id
 WHERE w.owner_user_id = $1 AND strpos(lower(coalesce(w.name, '')), lower($2)) > 0
), ranked AS (
 SELECT *, row_number() OVER (PARTITION BY kind ORDER BY lower(title), id) rn FROM matches
)
SELECT kind, id, title, context, snippet, organization_id, project_id, workspace_id,
 issue_id, archived FROM ranked WHERE rn <= 21 ORDER BY kind, rn
"#;

async fn handler(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, ErrorResponse> {
    let q = query.q.trim();
    if q.chars().count() > 200 {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "Search is limited to 200 characters",
        ));
    }
    if q.chars().count() < 2 {
        return Ok(Json(SearchResponse {
            results: vec![],
            truncated: false,
        }));
    }
    // SET LOCAL makes the bound effective in PostgreSQL itself, including after
    // an HTTP request is cancelled, and cannot leak into the pooled connection.
    let mut tx = state
        .pool()
        .begin()
        .await
        .map_err(|e| db_error(e, "Search unavailable"))?;
    sqlx::query("SET LOCAL statement_timeout = '3000ms'")
        .execute(&mut *tx)
        .await
        .map_err(|e| db_error(e, "Search unavailable"))?;
    let rows = sqlx::query_as::<_, SearchResult>(SEARCH)
        .bind(ctx.user.id)
        .bind(q)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| db_error(e, "Search unavailable; refine your query"))?;
    tx.commit()
        .await
        .map_err(|e| db_error(e, "Search unavailable"))?;
    let mut counts = std::collections::HashMap::new();
    let mut truncated = false;
    let results = rows
        .into_iter()
        .filter(|row| {
            let count = counts.entry(row.kind.clone()).or_insert(0);
            *count += 1;
            truncated |= *count > 20;
            *count <= 20
        })
        .collect();
    Ok(Json(SearchResponse { results, truncated }))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/global-search", get(handler))
}
