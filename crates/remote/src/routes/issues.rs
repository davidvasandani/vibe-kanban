use api_types::{
    CreateIssueRequest, DeleteResponse, Issue, ListIssuesQuery, ListIssuesResponse,
    MutationResponse, NotificationPayload, NotificationType, PullRequestStatus,
    ResolveLowDiskIssueRequest, ResolveLowDiskIssueResponse, SearchIssuesRequest,
    UpdateIssueRequest,
};
use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::post,
};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use uuid::Uuid;

use super::{
    error::{ErrorResponse, db_error},
    organization_members::ensure_project_access,
};
use crate::{
    AppState,
    auth::RequestContext,
    db::{
        get_txid, issue_followers::IssueFollowerRepository, issues::IssueRepository,
        project_statuses::ProjectStatusRepository, pull_requests::PullRequestRepository,
        workspaces::WorkspaceRepository,
    },
    mutation_definition::MutationBuilder,
    notifications::{
        collect_issue_recipients, send_debounced_issue_notifications, send_issue_notifications,
    },
};

/// Mutation definition for Issue - provides both router and TypeScript metadata.
pub fn mutation() -> MutationBuilder<Issue, CreateIssueRequest, UpdateIssueRequest> {
    MutationBuilder::new("issues")
        .list(list_issues)
        .get(get_issue)
        .create(create_issue)
        .update(update_issue)
        .delete(delete_issue)
}

/// Router for issue endpoints including bulk update
pub fn router() -> axum::Router<AppState> {
    mutation()
        .router()
        .route("/issues/search", post(search_issues))
        .route("/issues/low-disk", post(resolve_low_disk_issue))
        .route("/issues/bulk", post(bulk_update_issues))
}

#[instrument(name = "issues.resolve_low_disk", skip(state, ctx, payload), fields(project_id = %payload.project_id, node_id = %payload.node_id, user_id = %ctx.user.id))]
async fn resolve_low_disk_issue(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    headers: HeaderMap,
    Json(payload): Json<ResolveLowDiskIssueRequest>,
) -> Result<Json<ResolveLowDiskIssueResponse>, ErrorResponse> {
    if headers
        .get("x-client-type")
        .and_then(|value| value.to_str().ok())
        != Some("local-backend")
    {
        return Err(ErrorResponse::new(
            StatusCode::FORBIDDEN,
            "low-disk observations must come from the coordinator",
        ));
    }
    ensure_project_access(state.pool(), ctx.user.id, payload.project_id).await?;
    if payload.hostname.trim().is_empty() || payload.filesystems.is_empty() {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "low-disk observation is incomplete",
        ));
    }
    let response = IssueRepository::resolve_low_disk_issue(state.pool(), &payload, ctx.user.id)
        .await
        .map_err(|error| db_error(error, "failed to resolve low-disk issue"))?;
    if response.created
        && let Err(error) =
            IssueFollowerRepository::create(state.pool(), None, response.issue.id, ctx.user.id)
                .await
    {
        tracing::warn!(?error, issue_id = %response.issue.id, "failed to auto-follow low-disk issue");
    }
    Ok(Json(response))
}

async fn notify_issue_update_changes(
    state: &AppState,
    organization_id: Uuid,
    actor_user_id: Uuid,
    old_issue: &Issue,
    new_issue: &Issue,
) {
    let status_changed = old_issue.status_id != new_issue.status_id;
    let title_changed = old_issue.title != new_issue.title;
    let description_changed = old_issue.description != new_issue.description;
    let priority_changed = old_issue.priority != new_issue.priority;

    let needs_notification =
        status_changed || title_changed || description_changed || priority_changed;
    if !needs_notification {
        return;
    }

    let recipients =
        match collect_issue_recipients(state.pool(), organization_id, new_issue.id, actor_user_id)
            .await
        {
            Ok(recipients) => recipients,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    issue_id = %new_issue.id,
                    "failed to collect notification recipients"
                );
                vec![]
            }
        };

    if recipients.is_empty() {
        return;
    }

    if status_changed {
        let old_status_name =
            ProjectStatusRepository::find_by_id(state.pool(), old_issue.status_id)
                .await
                .ok()
                .flatten()
                .map(|s| s.name);
        let new_status_name =
            ProjectStatusRepository::find_by_id(state.pool(), new_issue.status_id)
                .await
                .ok()
                .flatten()
                .map(|s| s.name);

        send_issue_notifications(
            state.pool(),
            organization_id,
            actor_user_id,
            &recipients,
            new_issue,
            NotificationType::IssueStatusChanged,
            NotificationPayload {
                old_status_id: Some(old_issue.status_id),
                new_status_id: Some(new_issue.status_id),
                old_status_name,
                new_status_name,
                ..Default::default()
            },
            None,
            Some(new_issue.id),
        )
        .await;
    }

    if title_changed {
        send_debounced_issue_notifications(
            state.pool(),
            organization_id,
            actor_user_id,
            &recipients,
            new_issue,
            NotificationType::IssueTitleChanged,
            NotificationPayload {
                new_title: Some(new_issue.title.clone()),
                ..Default::default()
            },
            None,
            Some(new_issue.id),
        )
        .await;
    }

    if description_changed {
        send_debounced_issue_notifications(
            state.pool(),
            organization_id,
            actor_user_id,
            &recipients,
            new_issue,
            NotificationType::IssueDescriptionChanged,
            NotificationPayload::default(),
            None,
            Some(new_issue.id),
        )
        .await;
    }

    if priority_changed {
        send_debounced_issue_notifications(
            state.pool(),
            organization_id,
            actor_user_id,
            &recipients,
            new_issue,
            NotificationType::IssuePriorityChanged,
            NotificationPayload {
                old_priority: old_issue.priority,
                new_priority: new_issue.priority,
                ..Default::default()
            },
            None,
            Some(new_issue.id),
        )
        .await;
    }
}

/// Recognises the terminal issue statuses that trigger workspace archiving.
///
/// Statuses are per-project and user-customisable and carry no terminal flag,
/// so — like the existing behaviour did for "Done" — we identify them by
/// case-insensitive name. Returns the canonical label ("Done" / "Cancelled")
/// for a match, or `None` for any non-terminal status. The American spelling
/// "Canceled" is accepted and normalised to "Cancelled".
fn terminal_status_name(name: &str) -> Option<&'static str> {
    if name.eq_ignore_ascii_case("Done") {
        Some("Done")
    } else if name.eq_ignore_ascii_case("Cancelled") || name.eq_ignore_ascii_case("Canceled") {
        Some("Cancelled")
    } else {
        None
    }
}

/// When an issue moves into a terminal status ("Done" or "Cancelled"), archive
/// its still-active workspaces. When the status is "Done", emits a warning if
/// the issue's pull requests are not all merged, since archiving then hides work
/// that was never merged; "Cancelled" work is intentionally abandoned, so no
/// such warning is emitted for it.
///
/// Runs on the caller's transaction connection so the archive is committed
/// atomically with the issue status update and is covered by the same txid
/// the client waits on.
async fn archive_workspaces_for_terminal_issue(
    conn: &mut sqlx::PgConnection,
    old_issue: &Issue,
    new_issue: &Issue,
) -> Result<(), ErrorResponse> {
    // Only react when the status actually changes.
    if old_issue.status_id == new_issue.status_id {
        return Ok(());
    }

    let status = ProjectStatusRepository::find_by_id(&mut *conn, new_issue.status_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, issue_id = %new_issue.id, "failed to load status while archiving terminal issue");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?;
    let Some(terminal) = status
        .as_ref()
        .and_then(|status| terminal_status_name(&status.name))
    else {
        return Ok(());
    };

    let workspaces = WorkspaceRepository::list_active_by_issue_id(&mut *conn, new_issue.id)
        .await
        .map_err(|error| {
            tracing::error!(?error, issue_id = %new_issue.id, "failed to list workspaces while archiving terminal issue");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?;
    if workspaces.is_empty() {
        return Ok(());
    }

    // For "Done" only, warn once if the issue's work was never fully merged.
    // Pull requests are linked to issues, not reliably to individual workspaces
    // (the pull_requests.workspace_id column is not populated on creation), so
    // this is evaluated at the issue level. A lookup failure downgrades to "no
    // warning" rather than failing the request. "Cancelled" work is expected to
    // be discarded, so the unmerged warning would be noise there.
    if terminal == "Done" {
        let pull_requests = PullRequestRepository::list_by_issue(&mut *conn, new_issue.id)
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(?error, issue_id = %new_issue.id, "failed to load pull requests while archiving done issue");
                Vec::new()
            });
        let work_merged = !pull_requests.is_empty()
            && pull_requests
                .iter()
                .all(|pr| pr.status == PullRequestStatus::Merged);
        if !work_merged {
            tracing::warn!(
                issue_id = %new_issue.id,
                workspace_count = workspaces.len(),
                "archiving workspaces for issue marked Done, but its pull requests are not all merged"
            );
        }
    }

    WorkspaceRepository::archive_active_by_issue_id(&mut *conn, new_issue.id)
        .await
        .map_err(|error| {
            tracing::error!(?error, issue_id = %new_issue.id, "failed to archive workspaces for terminal issue");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?;

    tracing::info!(
        issue_id = %new_issue.id,
        status = terminal,
        workspace_count = workspaces.len(),
        "archived workspaces for issue moved to terminal status"
    );

    Ok(())
}

#[instrument(
    name = "issues.list_issues",
    skip(state, ctx),
    fields(project_id = %query.project_id, user_id = %ctx.user.id)
)]
async fn list_issues(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Query(query): Query<ListIssuesQuery>,
) -> Result<Json<ListIssuesResponse>, ErrorResponse> {
    let project_id = query.project_id;
    ensure_project_access(state.pool(), ctx.user.id, project_id).await?;
    let request = SearchIssuesRequest {
        project_id,
        status_id: None,
        status_ids: None,
        priority: None,
        parent_issue_id: None,
        search: None,
        simple_id: None,
        assignee_user_id: None,
        tag_id: None,
        tag_ids: None,
        sort_field: None,
        sort_direction: None,
        limit: None,
        offset: None,
    };

    let response = IssueRepository::search(state.pool(), &request)
        .await
        .map_err(|error| {
            tracing::error!(?error, project_id = %project_id, "failed to list issues");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to list issues")
        })?;

    Ok(Json(response))
}

#[instrument(
    name = "issues.search_issues",
    skip(state, ctx, payload),
    fields(project_id = %payload.project_id, user_id = %ctx.user.id)
)]
async fn search_issues(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<SearchIssuesRequest>,
) -> Result<Json<ListIssuesResponse>, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, payload.project_id).await?;

    let response = IssueRepository::search(state.pool(), &payload)
        .await
        .map_err(|error| {
            tracing::error!(?error, project_id = %payload.project_id, "failed to search issues");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to search issues")
        })?;

    Ok(Json(response))
}

#[instrument(
    name = "issues.get_issue",
    skip(state, ctx),
    fields(issue_id = %issue_id, user_id = %ctx.user.id)
)]
async fn get_issue(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(issue_id): Path<Uuid>,
) -> Result<Json<Issue>, ErrorResponse> {
    let issue = IssueRepository::find_by_id(state.pool(), issue_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, %issue_id, "failed to load issue");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load issue")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "issue not found"))?;

    ensure_project_access(state.pool(), ctx.user.id, issue.project_id).await?;

    Ok(Json(issue))
}

#[instrument(
    name = "issues.create_issue",
    skip(state, ctx, payload),
    fields(project_id = %payload.project_id, user_id = %ctx.user.id)
)]
async fn create_issue(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<CreateIssueRequest>,
) -> Result<Json<MutationResponse<Issue>>, ErrorResponse> {
    let organization_id =
        ensure_project_access(state.pool(), ctx.user.id, payload.project_id).await?;

    let has_parent = payload.parent_issue_id.is_some();
    let has_description = payload.description.is_some();
    let priority = payload.priority;
    let parent_issue_id = payload.parent_issue_id;

    let response = IssueRepository::create(
        state.pool(),
        payload.id,
        payload.project_id,
        payload.status_id,
        payload.title,
        payload.description,
        payload.priority,
        payload.start_date,
        payload.target_date,
        payload.completed_at,
        payload.sort_order,
        payload.parent_issue_id,
        payload.parent_issue_sort_order,
        payload.extension_metadata,
        ctx.user.id,
    )
    .await
    .map_err(|error| {
        tracing::error!(?error, "failed to create issue");
        db_error(error, "failed to create issue")
    })?;

    // Auto-follow: the creator should receive notifications for all activity on this issue.
    if let Err(e) =
        IssueFollowerRepository::create(state.pool(), None, response.data.id, ctx.user.id).await
    {
        tracing::warn!(?e, issue_id = %response.data.id, "failed to auto-follow issue for creator");
    }

    if let Some(analytics) = state.analytics() {
        analytics.track(
            ctx.user.id,
            "issue_created",
            serde_json::json!({
                "issue_id": response.data.id,
                "project_id": response.data.project_id,
                "organization_id": organization_id,
                "has_description": has_description,
                "has_parent": has_parent,
                "priority": format!("{:?}", priority),
            }),
        );

        if let Some(parent_id) = parent_issue_id {
            analytics.track(
                ctx.user.id,
                "subtask_created",
                serde_json::json!({
                    "issue_id": response.data.id,
                    "parent_issue_id": parent_id,
                    "project_id": response.data.project_id,
                    "organization_id": organization_id,
                }),
            );
        }
    }

    Ok(Json(response))
}

#[instrument(
    name = "issues.update_issue",
    skip(state, ctx, payload),
    fields(issue_id = %issue_id, user_id = %ctx.user.id)
)]
async fn update_issue(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(issue_id): Path<Uuid>,
    Json(payload): Json<UpdateIssueRequest>,
) -> Result<Json<MutationResponse<Issue>>, ErrorResponse> {
    let issue = IssueRepository::find_by_id(state.pool(), issue_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, %issue_id, "failed to load issue");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load issue")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "issue not found"))?;

    let organization_id =
        ensure_project_access(state.pool(), ctx.user.id, issue.project_id).await?;

    let mut tx = crate::db::begin_tx(state.pool()).await.map_err(|error| {
        tracing::error!(?error, "failed to begin transaction");
        ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
    })?;

    let data = IssueRepository::update(
        &mut *tx,
        issue_id,
        payload.status_id,
        payload.title,
        payload.description,
        payload.priority,
        payload.start_date,
        payload.target_date,
        payload.completed_at,
        payload.sort_order,
        payload.parent_issue_id,
        payload.parent_issue_sort_order,
        payload.extension_metadata,
    )
    .await
    .map_err(|error| {
        tracing::error!(?error, "failed to update issue");
        ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
    })?;

    // Archive workspaces within the same transaction so the change is covered
    // by the returned txid the client waits on.
    archive_workspaces_for_terminal_issue(&mut tx, &issue, &data).await?;

    let txid = get_txid(&mut *tx).await.map_err(|error| {
        tracing::error!(?error, "failed to get txid");
        ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
    })?;

    tx.commit().await.map_err(|error| {
        tracing::error!(?error, "failed to commit transaction");
        ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
    })?;

    notify_issue_update_changes(&state, organization_id, ctx.user.id, &issue, &data).await;

    Ok(Json(MutationResponse { data, txid }))
}

#[instrument(
    name = "issues.delete_issue",
    skip(state, ctx),
    fields(issue_id = %issue_id, user_id = %ctx.user.id)
)]
async fn delete_issue(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(issue_id): Path<Uuid>,
) -> Result<Json<DeleteResponse>, ErrorResponse> {
    let issue = IssueRepository::find_by_id(state.pool(), issue_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, %issue_id, "failed to load issue");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load issue")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "issue not found"))?;

    let organization_id =
        ensure_project_access(state.pool(), ctx.user.id, issue.project_id).await?;

    let recipients = match collect_issue_recipients(
        state.pool(),
        organization_id,
        issue.id,
        ctx.user.id,
    )
    .await
    {
        Ok(recipients) => recipients,
        Err(error) => {
            tracing::warn!(
                ?error,
                issue_id = %issue.id,
                "failed to collect notification recipients"
            );
            vec![]
        }
    };

    let response = IssueRepository::delete(state.pool(), issue_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to delete issue");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?;

    send_issue_notifications(
        state.pool(),
        organization_id,
        ctx.user.id,
        &recipients,
        &issue,
        NotificationType::IssueDeleted,
        NotificationPayload::default(),
        None,
        None,
    )
    .await;

    Ok(Json(response))
}

// =============================================================================
// Bulk Update
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct BulkUpdateIssueItem {
    pub id: Uuid,
    #[serde(flatten)]
    pub changes: UpdateIssueRequest,
}

#[derive(Debug, Deserialize)]
pub struct BulkUpdateIssuesRequest {
    pub updates: Vec<BulkUpdateIssueItem>,
}

#[derive(Debug, Serialize)]
pub struct BulkUpdateIssuesResponse {
    pub data: Vec<Issue>,
    pub txid: i64,
}

#[instrument(
    name = "issues.bulk_update",
    skip(state, ctx, payload),
    fields(user_id = %ctx.user.id, count = payload.updates.len())
)]
async fn bulk_update_issues(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<BulkUpdateIssuesRequest>,
) -> Result<Json<BulkUpdateIssuesResponse>, ErrorResponse> {
    if payload.updates.is_empty() {
        return Ok(Json(BulkUpdateIssuesResponse {
            data: vec![],
            txid: 0,
        }));
    }

    // Get first issue to determine project_id for access check
    let first_issue = IssueRepository::find_by_id(state.pool(), payload.updates[0].id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to find first issue");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to find issue")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "issue not found"))?;

    let project_id = first_issue.project_id;
    let organization_id = ensure_project_access(state.pool(), ctx.user.id, project_id).await?;

    let mut tx = crate::db::begin_tx(state.pool()).await.map_err(|error| {
        tracing::error!(?error, "failed to begin transaction");
        ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
    })?;

    let mut results = Vec::with_capacity(payload.updates.len());
    let mut notification_pairs = Vec::with_capacity(payload.updates.len());

    for item in payload.updates {
        // Verify issue belongs to the same project
        let issue = IssueRepository::find_by_id(&mut *tx, item.id)
            .await
            .map_err(|error| {
                tracing::error!(?error, issue_id = %item.id, "failed to find issue");
                ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to find issue")
            })?
            .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "issue not found"))?;

        if issue.project_id != project_id {
            return Err(ErrorResponse::new(
                StatusCode::BAD_REQUEST,
                "all issues must belong to the same project",
            ));
        }

        // Update the issue
        let updated = IssueRepository::update(
            &mut *tx,
            item.id,
            item.changes.status_id,
            item.changes.title,
            item.changes.description,
            item.changes.priority,
            item.changes.start_date,
            item.changes.target_date,
            item.changes.completed_at,
            item.changes.sort_order,
            item.changes.parent_issue_id,
            item.changes.parent_issue_sort_order,
            item.changes.extension_metadata,
        )
        .await
        .map_err(|error| {
            tracing::error!(?error, issue_id = %item.id, "failed to update issue");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to update issue")
        })?;

        // Archive workspaces in the same transaction as the status change so
        // they are covered by the returned txid.
        archive_workspaces_for_terminal_issue(&mut tx, &issue, &updated).await?;

        notification_pairs.push((issue, updated.clone()));
        results.push(updated);
    }

    let txid = get_txid(&mut *tx).await.map_err(|error| {
        tracing::error!(?error, "failed to get txid");
        ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
    })?;
    tx.commit().await.map_err(|error| {
        tracing::error!(?error, "failed to commit transaction");
        ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
    })?;

    for (old_issue, new_issue) in &notification_pairs {
        notify_issue_update_changes(&state, organization_id, ctx.user.id, old_issue, new_issue)
            .await;
    }

    Ok(Json(BulkUpdateIssuesResponse {
        data: results,
        txid,
    }))
}

#[cfg(test)]
mod tests {
    use super::terminal_status_name;

    #[test]
    fn recognizes_done_case_insensitively() {
        assert_eq!(terminal_status_name("Done"), Some("Done"));
        assert_eq!(terminal_status_name("done"), Some("Done"));
        assert_eq!(terminal_status_name("DONE"), Some("Done"));
    }

    #[test]
    fn recognizes_cancelled_and_american_spelling() {
        assert_eq!(terminal_status_name("Cancelled"), Some("Cancelled"));
        assert_eq!(terminal_status_name("cancelled"), Some("Cancelled"));
        assert_eq!(terminal_status_name("Canceled"), Some("Cancelled"));
        assert_eq!(terminal_status_name("CANCELED"), Some("Cancelled"));
    }

    #[test]
    fn rejects_non_terminal_statuses() {
        assert_eq!(terminal_status_name("In progress"), None);
        assert_eq!(terminal_status_name("In review"), None);
        assert_eq!(terminal_status_name("Backlog"), None);
        assert_eq!(terminal_status_name("To do"), None);
        assert_eq!(terminal_status_name(""), None);
        assert_eq!(terminal_status_name("Do"), None);
    }
}
