//! Local durable continuation content; admission remains owned by the shared dispatcher.
use axum::{Extension, Json, extract::{Path, State}};
use chrono::{DateTime, Utc};
use db::models::{session::Session, workspace::Workspace, wakeup::{Wakeup, WakeupSummary, MAX_DELAY_SECS}};
use deployment::Deployment;
use serde::Deserialize;
use ts_rs::TS;
use uuid::Uuid;
use utils::response::ApiResponse;
use crate::{DeploymentImpl, error::ApiError};

#[derive(Debug, Deserialize, TS)]
pub struct ScheduleWakeupRequest {
    pub session_id: Uuid,
    pub fire_at: Option<DateTime<Utc>>,
    pub delay_secs: Option<u32>,
    pub on_fire_prompt: String,
}

pub fn deadline(request: &ScheduleWakeupRequest, now: DateTime<Utc>) -> Result<DateTime<Utc>, ApiError> {
    let at = match (request.fire_at, request.delay_secs) {
        (Some(at), None) => at,
        (None, Some(seconds)) if seconds > 0 => now + chrono::Duration::seconds(seconds.into()),
        _ => return Err(ApiError::BadRequest("Supply exactly one timezone-aware fire_at or positive delay_secs".into())),
    };
    if at <= now || at > now + chrono::Duration::seconds(MAX_DELAY_SECS) {
        return Err(ApiError::BadRequest("Wake-up must be in the future and within seven days".into()));
    }
    validate_prompt(&request.on_fire_prompt)?;
    Ok(at)
}

pub fn validate_prompt(prompt: &str) -> Result<(), ApiError> {
    if prompt.trim().is_empty() || prompt.len() > 16_384 {
        return Err(ApiError::BadRequest("Wake-up prompt must contain 1–16384 bytes of nonblank text".into()));
    }
    Ok(())
}

pub async fn validate_session(pool: &sqlx::SqlitePool, workspace: &Workspace, id: Uuid) -> Result<Session, ApiError> {
    if workspace.archived || workspace.worktree_deleted {
        return Err(ApiError::BadRequest("Cannot schedule in an archived or deleted workspace".into()));
    }
    let session = Session::find_by_id(pool, id).await?
        .filter(|s| s.workspace_id == workspace.id)
        .ok_or_else(|| ApiError::BadRequest("Session does not belong to workspace".into()))?;
    Ok(session)
}

pub async fn schedule(Extension(workspace): Extension<Workspace>, State(deployment): State<DeploymentImpl>, Json(request): Json<ScheduleWakeupRequest>) -> Result<Json<ApiResponse<Wakeup>>, ApiError> {
    let now = Utc::now();
    let fire_at = deadline(&request, now)?;
    validate_session(&deployment.db().pool, &workspace, request.session_id).await?;
    let row = Wakeup { id: Uuid::new_v4(), workspace_id: workspace.id, session_id: request.session_id,
        fire_at, prompt: request.on_fire_prompt, poller_id: None, receipt_path: None,
        trigger_context: None, created_at: now };
    if !row.insert(&deployment.db().pool).await? {
        return Err(ApiError::BadRequest("Wake-up limit reached: eight pending or 32 total per session".into()));
    }
    Ok(Json(ApiResponse::success(row)))
}

pub async fn list(Extension(workspace): Extension<Workspace>, State(deployment): State<DeploymentImpl>) -> Result<Json<ApiResponse<Vec<WakeupSummary>>>, ApiError> {
    Ok(Json(ApiResponse::success(Wakeup::list(&deployment.db().pool, workspace.id).await?)))
}

pub async fn cancel(Extension(workspace): Extension<Workspace>, State(deployment): State<DeploymentImpl>, Path(id): Path<Uuid>) -> Result<Json<ApiResponse<bool>>, ApiError> {
    let pool = &deployment.db().pool;
    let row = Wakeup::find(pool, id).await?.filter(|w| w.workspace_id == workspace.id)
        .ok_or_else(|| ApiError::BadRequest("Wake-up does not belong to workspace".into()))?;
    if !Wakeup::cancel(pool, row.id, "Cancelled by operator").await? {
        return Err(ApiError::BadRequest("Wake-up already claimed or finished; cancellation was not applied".into()));
    }
    Ok(Json(ApiResponse::success(true)))
}

/// Reconcile invalid targets even before their deadline. IDs in wakeups survive
/// cascading target deletion, unlike the nullable target IDs in delivery rows.
pub async fn reconcile(pool: &sqlx::SqlitePool) -> Result<(), sqlx::Error> {
    let pending: Vec<Wakeup> = sqlx::query_as("SELECT w.* FROM wakeups w JOIN issue_comment_deliveries d ON d.comment_id = w.id WHERE d.state = 'pending'").fetch_all(pool).await?;
    for row in pending {
        let workspace = Workspace::find_by_id(pool, row.workspace_id).await?;
        let session = Session::find_by_id(pool, row.session_id).await?;
        let reason = match workspace {
            None => Some("Workspace deleted"),
            Some(w) if w.archived || w.worktree_deleted => Some("Workspace archived or worktree deleted"),
            _ if session.is_none() => Some("Session deleted"),
            _ => None,
        };
        if let Some(reason) = reason {
            Wakeup::cancel(pool, row.id, reason).await?;
            continue;
        }
        if let Some(path) = &row.receipt_path {
            if row.trigger_context.is_some() { continue; }
            // Receipt is atomically renamed only by the successful predicate path.
            // Bound reads regardless of file size (workspace content is untrusted).
            use tokio::io::AsyncReadExt;
            if let Ok(file) = tokio::fs::File::open(path).await {
                let mut bytes = Vec::new();
                if file.take(8192).read_to_end(&mut bytes).await.is_ok() {
                    let context = String::from_utf8_lossy(&bytes);
                    let now = Utc::now();
                    let mut tx = pool.begin().await?;
                    sqlx::query("UPDATE wakeups SET trigger_context = ?, fire_at = ? WHERE id = ? AND trigger_context IS NULL")
                        .bind(context.as_ref()).bind(now).bind(row.id).execute(&mut *tx).await?;
                    sqlx::query("UPDATE issue_comment_deliveries SET next_attempt_at = ? WHERE comment_id = ? AND state = 'pending'")
                        .bind(now).bind(row.id).execute(&mut *tx).await?;
                    tx.commit().await?;
                    continue;
                }
            }
            if let Some(id) = row.poller_id {
                let process = db::models::execution_process::ExecutionProcess::find_by_id(pool, id).await?;
                if process.is_none_or(|p| p.status != db::models::execution_process::ExecutionProcessStatus::Running) {
                    Wakeup::cancel(pool, row.id, "Poller stopped without a successful completion receipt").await?;
                    continue;
                }
            }
            if Utc::now() >= row.fire_at {
                Wakeup::cancel(pool, row.id, "Completion predicate did not produce a receipt before the wake-up deadline").await?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_validation_requires_one_future_bounded_deadline() {
        let now = Utc::now();
        let mut request = ScheduleWakeupRequest { session_id: Uuid::new_v4(), fire_at: None, delay_secs: Some(60), on_fire_prompt: "continue".into() };
        assert_eq!(deadline(&request, now).unwrap(), now + chrono::Duration::seconds(60));
        request.fire_at = Some(now + chrono::Duration::seconds(60));
        assert!(deadline(&request, now).is_err());
        request.delay_secs = None;
        assert!(deadline(&request, now).is_ok());
        request.fire_at = Some(now);
        assert!(deadline(&request, now).is_err());
        request.fire_at = None;
        request.delay_secs = Some(MAX_DELAY_SECS as u32 + 1);
        assert!(deadline(&request, now).is_err());
        request.delay_secs = Some(1);
        request.on_fire_prompt = "  ".into();
        assert!(deadline(&request, now).is_err());
        assert!(serde_json::from_value::<ScheduleWakeupRequest>(serde_json::json!({
            "session_id": Uuid::new_v4(), "fire_at": "2026-10-25T22:05:00", "on_fire_prompt": "go"
        })).is_err());
    }
}
