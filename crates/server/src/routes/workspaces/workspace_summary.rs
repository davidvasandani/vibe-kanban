use std::collections::HashMap;

use axum::{Json, extract::State, response::Json as ResponseJson};
use chrono::{DateTime, Duration, Utc};
use db::models::{
    coding_agent_turn::CodingAgentTurn,
    execution_process::{ExecutionProcess, ExecutionProcessStatus, LatestProcessInfo},
    merge::MergeStatus,
    pull_request::PullRequest,
    worker_node::WorkerNode,
    workspace::{Workspace, WorkspacePlacement, WorkspacePlacementState},
};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

/// Skip bulk `git status` for workspaces whose last real activity is older than this.
/// Filesystem mtime is not used: the worktree poller keeps those timestamps fresh.
fn git_status_idle_after() -> Duration {
    Duration::days(14)
}

/// Request for fetching workspace summaries
#[derive(Debug, Deserialize, Serialize, TS)]
pub struct WorkspaceSummaryRequest {
    pub archived: bool,
}

/// Summary info for a single workspace
#[derive(Debug, Serialize, TS)]
pub struct WorkspaceSummary {
    pub workspace_id: Uuid,
    /// Session ID of the latest execution process
    pub latest_session_id: Option<Uuid>,
    /// Is a tool approval currently pending?
    pub has_pending_approval: bool,
    /// Number of files with changes
    pub files_changed: Option<usize>,
    /// Total lines added across all files
    pub lines_added: Option<usize>,
    /// Total lines removed across all files
    pub lines_removed: Option<usize>,
    /// When the latest execution process completed
    #[ts(optional)]
    pub latest_process_completed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Status of the latest execution process
    pub latest_process_status: Option<ExecutionProcessStatus>,
    /// Is a dev server currently running?
    pub has_running_dev_server: bool,
    /// Does this workspace have unseen coding agent turns?
    pub has_unseen_turns: bool,
    /// PR status for this workspace (if any PR exists)
    pub pr_status: Option<MergeStatus>,
    /// PR number for this workspace (if any PR exists)
    pub pr_number: Option<i64>,
    /// PR URL for this workspace (if any PR exists)
    pub pr_url: Option<String>,
    pub affinity: WorkspaceAffinitySummary,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct WorkspaceAffinitySummary {
    pub kind: WorkspaceAffinityKind,
    pub placement_state: WorkspacePlacementState,
    pub worker_node_id: Option<Uuid>,
    pub worker_hostname: Option<String>,
    pub requested_worker_node_id: Option<Uuid>,
    pub requested_worker_hostname: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[ts(use_ts_enum)]
pub enum WorkspaceAffinityKind {
    Local,
    Automatic,
    Worker,
    Unassigned,
}

fn affinity_kind(placement: Option<&WorkspacePlacement>) -> WorkspaceAffinityKind {
    let Some(placement) = placement else {
        return WorkspaceAffinityKind::Local;
    };
    if placement.placement_state == WorkspacePlacementState::Local {
        WorkspaceAffinityKind::Local
    } else if placement.requested_worker_node_id.is_some() {
        WorkspaceAffinityKind::Worker
    } else if placement.worker_node_id.is_some() {
        WorkspaceAffinityKind::Automatic
    } else {
        WorkspaceAffinityKind::Unassigned
    }
}

/// Response containing summaries for requested workspaces
#[derive(Debug, Serialize, TS)]
pub struct WorkspaceSummaryResponse {
    pub summaries: Vec<WorkspaceSummary>,
}

#[derive(Debug, Clone, Default, Serialize, TS)]
pub struct DiffStats {
    pub files_changed: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
}

/// Last time someone actually used this workspace.
///
/// Prefer the latest execution-process completion time (`LatestProcessInfo.completed_at`
/// from `ExecutionProcess::find_latest_for_workspaces`). A still-running process is
/// treated as current activity. If there is no process, fall back to
/// `workspace.updated_at` (touched on editor open / workspace access — not worktree mtime).
fn last_workspace_activity(
    workspace_updated_at: DateTime<Utc>,
    latest: Option<&LatestProcessInfo>,
    now: DateTime<Utc>,
) -> DateTime<Utc> {
    match latest {
        Some(info) if info.status == ExecutionProcessStatus::Running => now,
        Some(info) => info.completed_at.unwrap_or(workspace_updated_at),
        None => workspace_updated_at,
    }
}

fn should_skip_idle_git_status(last_activity: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    now.signed_duration_since(last_activity) > git_status_idle_after()
}

/// Fetch summary information for workspaces filtered by archived status.
/// This endpoint returns data that cannot be efficiently included in the streaming endpoint.
#[axum::debug_handler]
pub async fn get_workspace_summaries(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<WorkspaceSummaryRequest>,
) -> Result<ResponseJson<ApiResponse<WorkspaceSummaryResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let archived = request.archived;

    // 1. Fetch all workspaces with the given archived status
    let workspaces: Vec<Workspace> = Workspace::find_all_with_status(pool, Some(archived), None)
        .await?
        .into_iter()
        .map(|ws| ws.workspace)
        .collect();

    if workspaces.is_empty() {
        return Ok(ResponseJson(ApiResponse::success(
            WorkspaceSummaryResponse { summaries: vec![] },
        )));
    }

    // 2. Fetch latest process info for workspaces with this archived status
    let latest_processes = ExecutionProcess::find_latest_for_workspaces(pool, archived).await?;

    // 3. Check which workspaces have running dev servers
    let dev_server_workspaces =
        ExecutionProcess::find_workspaces_with_running_dev_servers(pool, archived).await?;

    // 4. Check pending approvals for running processes
    let running_ep_ids: Vec<_> = latest_processes
        .values()
        .filter(|info| info.status == ExecutionProcessStatus::Running)
        .map(|info| info.execution_process_id)
        .collect();
    let pending_approval_eps = deployment
        .approvals()
        .get_pending_execution_process_ids(&running_ep_ids);

    // 5. Check which workspaces have unseen coding agent turns
    let unseen_workspaces = CodingAgentTurn::find_workspaces_with_unseen(pool, archived).await?;

    // 6. Get PR status for each workspace
    let pr_statuses = PullRequest::get_latest_for_workspaces(pool, archived).await?;

    // 7. Resolve placement for every row with two bulk reads (never N+1).
    let placements: HashMap<Uuid, WorkspacePlacement> =
        WorkspacePlacement::find_all_by_archived(pool, archived)
            .await?
            .into_iter()
            .map(|placement| (placement.workspace_id, placement))
            .collect();
    let worker_hostnames: HashMap<Uuid, String> = WorkerNode::fetch_all(pool)
        .await?
        .into_iter()
        .map(|worker| (worker.id, worker.hostname))
        .collect();

    // 8. Compute diff stats for each workspace (in parallel).
    // Skip workspaces idle > 14 days so the sidebar poll does not `git status`
    // every stale worktree. On-demand single-workspace status is unchanged.
    let now = Utc::now();
    let diff_futures: Vec<_> = workspaces
        .iter()
        .map(|ws| {
            let workspace = ws.clone();
            let deployment = deployment.clone();
            let skip_idle = should_skip_idle_git_status(
                last_workspace_activity(ws.updated_at, latest_processes.get(&ws.id), now),
                now,
            );
            async move {
                if workspace.container_ref.is_some() && !skip_idle {
                    compute_workspace_diff_stats(&deployment, &workspace)
                        .await
                        .map(|stats| (workspace.id, stats))
                } else {
                    None
                }
            }
        })
        .collect();

    let diff_results: Vec<Option<(Uuid, DiffStats)>> =
        futures_util::future::join_all(diff_futures).await;
    let diff_stats: HashMap<Uuid, DiffStats> = diff_results.into_iter().flatten().collect();

    // 9. Assemble response
    let summaries: Vec<WorkspaceSummary> = workspaces
        .iter()
        .map(|ws| {
            let id = ws.id;
            let latest = latest_processes.get(&id);
            let has_pending = latest
                .map(|p| pending_approval_eps.contains(&p.execution_process_id))
                .unwrap_or(false);
            let stats = diff_stats.get(&id);
            let placement = placements.get(&id);
            let placement_state = placement
                .map(|placement| placement.placement_state)
                .unwrap_or(WorkspacePlacementState::Local);
            let worker_node_id = placement.and_then(|placement| placement.worker_node_id);
            let requested_worker_node_id =
                placement.and_then(|placement| placement.requested_worker_node_id);
            let kind = affinity_kind(placement);

            WorkspaceSummary {
                workspace_id: id,
                latest_session_id: latest.map(|p| p.session_id),
                has_pending_approval: has_pending,
                files_changed: stats.map(|s| s.files_changed),
                lines_added: stats.map(|s| s.lines_added),
                lines_removed: stats.map(|s| s.lines_removed),
                latest_process_completed_at: latest.and_then(|p| p.completed_at),
                latest_process_status: latest.map(|p| p.status.clone()),
                has_running_dev_server: dev_server_workspaces.contains(&id),
                has_unseen_turns: unseen_workspaces.contains(&id),
                pr_status: pr_statuses.get(&id).map(|pr| pr.pr_status.clone()),
                pr_number: pr_statuses.get(&id).map(|pr| pr.pr_number),
                pr_url: pr_statuses.get(&id).map(|pr| pr.pr_url.clone()),
                affinity: WorkspaceAffinitySummary {
                    kind,
                    placement_state,
                    worker_node_id,
                    worker_hostname: worker_node_id
                        .and_then(|worker_id| worker_hostnames.get(&worker_id).cloned()),
                    requested_worker_node_id,
                    requested_worker_hostname: requested_worker_node_id
                        .and_then(|worker_id| worker_hostnames.get(&worker_id).cloned()),
                },
            }
        })
        .collect();

    Ok(ResponseJson(ApiResponse::success(
        WorkspaceSummaryResponse { summaries },
    )))
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};
    use db::models::{
        execution_process::{ExecutionProcessStatus, LatestProcessInfo},
        workspace::{WorkspacePlacement, WorkspacePlacementState},
    };
    use uuid::Uuid;

    use super::{
        WorkspaceAffinityKind, affinity_kind, last_workspace_activity, should_skip_idle_git_status,
    };

    fn placement(
        state: WorkspacePlacementState,
        worker_node_id: Option<Uuid>,
        requested_worker_node_id: Option<Uuid>,
    ) -> WorkspacePlacement {
        WorkspacePlacement {
            workspace_id: Uuid::new_v4(),
            worker_node_id,
            placement_state: state,
            placed_at: None,
            placement_reason: None,
            requested_worker_node_id,
            placement_constraints: None,
        }
    }

    fn latest_process(
        status: ExecutionProcessStatus,
        completed_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> LatestProcessInfo {
        LatestProcessInfo {
            workspace_id: Uuid::new_v4(),
            execution_process_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            status,
            completed_at,
        }
    }

    #[test]
    fn classifies_local_automatic_explicit_and_unassigned_affinity() {
        let worker_id = Uuid::new_v4();
        assert_eq!(affinity_kind(None), WorkspaceAffinityKind::Local);
        assert_eq!(
            affinity_kind(Some(&placement(
                WorkspacePlacementState::Ready,
                Some(worker_id),
                None,
            ))),
            WorkspaceAffinityKind::Automatic
        );
        assert_eq!(
            affinity_kind(Some(&placement(
                WorkspacePlacementState::Ready,
                Some(worker_id),
                Some(worker_id),
            ))),
            WorkspaceAffinityKind::Worker
        );
        assert_eq!(
            affinity_kind(Some(&placement(
                WorkspacePlacementState::Failed,
                None,
                None,
            ))),
            WorkspaceAffinityKind::Unassigned
        );
    }

    #[test]
    fn skips_git_status_when_last_process_completed_over_14_days_ago() {
        let now = Utc.with_ymd_and_hms(2026, 8, 15, 21, 0, 0).unwrap();
        let completed = now - Duration::days(15);
        // Even a recently-touched workspace.updated_at is ignored when a process exists.
        let updated_at = now - Duration::hours(1);
        let latest = latest_process(ExecutionProcessStatus::Completed, Some(completed));
        let last = last_workspace_activity(updated_at, Some(&latest), now);
        assert_eq!(last, completed);
        assert!(should_skip_idle_git_status(last, now));
    }

    #[test]
    fn does_not_skip_git_status_when_last_process_completed_within_14_days() {
        let now = Utc.with_ymd_and_hms(2026, 8, 15, 21, 0, 0).unwrap();
        let completed = now - Duration::days(14);
        let updated_at = now - Duration::days(30);
        let latest = latest_process(ExecutionProcessStatus::Completed, Some(completed));
        let last = last_workspace_activity(updated_at, Some(&latest), now);
        assert!(!should_skip_idle_git_status(last, now));
    }

    #[test]
    fn does_not_skip_git_status_while_latest_process_is_running() {
        let now = Utc.with_ymd_and_hms(2026, 8, 15, 21, 0, 0).unwrap();
        let updated_at = now - Duration::days(40);
        let latest = latest_process(ExecutionProcessStatus::Running, None);
        let last = last_workspace_activity(updated_at, Some(&latest), now);
        assert_eq!(last, now);
        assert!(!should_skip_idle_git_status(last, now));
    }

    #[test]
    fn falls_back_to_workspace_updated_at_when_no_process() {
        let now = Utc.with_ymd_and_hms(2026, 8, 15, 21, 0, 0).unwrap();
        let stale = now - Duration::days(15);
        let recent = now - Duration::days(2);
        assert!(should_skip_idle_git_status(
            last_workspace_activity(stale, None, now),
            now
        ));
        assert!(!should_skip_idle_git_status(
            last_workspace_activity(recent, None, now),
            now
        ));
    }
}

/// Compute diff stats for a workspace.
pub async fn compute_workspace_diff_stats(
    deployment: &DeploymentImpl,
    workspace: &Workspace,
) -> Option<DiffStats> {
    let stats = services::services::diff_stream::compute_diff_stats(
        &deployment.db().pool,
        deployment.git(),
        workspace,
    )
    .await?;

    Some(DiffStats {
        files_changed: stats.files_changed,
        lines_added: stats.lines_added,
        lines_removed: stats.lines_removed,
    })
}
