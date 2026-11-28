use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use axum::{
    Extension, Json, Router,
    extract::State,
    response::{IntoResponse, Json as ResponseJson},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use db::models::{
    merge::{Merge, MergeStatus, PrMerge, PullRequestInfo},
    repo::{Repo, RepoError},
    workspace::Workspace,
    workspace_repo::WorkspaceRepo,
};
use deployment::Deployment;
use git::{ConflictOp, GitCliError, GitServiceError};
use serde::{Deserialize, Serialize};
use services::services::{
    container::ContainerService, diff_stream, github_credentials, remote_sync,
};
use ts_rs::TS;
use utils::response::ApiResponse;
use uuid::Uuid;
use workspace_manager::{SourceMirror, SourceMirrorFailure, SourceMirrorFailureKind};

use super::streams::{DiffStreamQuery, stream_workspace_diff_ws};
use crate::{DeploymentImpl, error::ApiError, middleware::signed_ws::SignedWsUpgrade};

#[derive(Debug, Deserialize, Serialize, TS)]
pub struct RebaseWorkspaceRequest {
    pub repo_id: Uuid,
    pub old_base_branch: Option<String>,
    pub new_base_branch: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, TS)]
pub struct AbortConflictsRequest {
    pub repo_id: Uuid,
}

#[derive(Debug, Deserialize, Serialize, TS)]
pub struct ContinueRebaseRequest {
    pub repo_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type", rename_all = "snake_case")]
pub enum GitOperationError {
    MergeConflicts {
        message: String,
        op: ConflictOp,
        conflicted_files: Vec<String>,
        target_branch: String,
    },
    RebaseInProgress,
}

#[derive(Debug, Deserialize, Serialize, TS)]
pub struct MergeWorkspaceRequest {
    pub repo_id: Uuid,
}

#[derive(Debug, Deserialize, Serialize, TS)]
pub struct PushWorkspaceRequest {
    pub repo_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type", rename_all = "snake_case")]
pub enum PushError {
    ForcePushRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct BranchStatus {
    pub commits_behind: Option<usize>,
    pub commits_ahead: Option<usize>,
    pub has_uncommitted_changes: Option<bool>,
    pub head_oid: Option<String>,
    pub uncommitted_count: Option<usize>,
    pub untracked_count: Option<usize>,
    pub target_branch_name: String,
    pub remote_commits_behind: Option<usize>,
    pub remote_commits_ahead: Option<usize>,
    pub merges: Vec<Merge>,
    pub is_rebase_in_progress: bool,
    pub conflict_op: Option<ConflictOp>,
    pub conflicted_files: Vec<String>,
    pub is_target_remote: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct RepoBranchStatus {
    pub repo_id: Uuid,
    pub repo_name: String,
    #[serde(flatten)]
    pub status: BranchStatus,
    /// Why the background mirror of this workspace's branch into the
    /// operator's registered checkout is failing, if it is. `None` when the
    /// mirror is healthy, disabled, or has not run since the server started.
    pub source_mirror_error: Option<SourceMirrorError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum SourceMirrorErrorKind {
    Deterministic,
    Transient,
    Timeout,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct SourceMirrorError {
    pub kind: SourceMirrorErrorKind,
    /// Includes a bounded excerpt of git's own error output.
    pub message: String,
    pub attempts: u32,
    pub last_attempt_at: DateTime<Utc>,
    pub next_attempt_at: DateTime<Utc>,
}

impl From<SourceMirrorFailure> for SourceMirrorError {
    fn from(failure: SourceMirrorFailure) -> Self {
        Self {
            kind: match failure.kind {
                SourceMirrorFailureKind::Deterministic => SourceMirrorErrorKind::Deterministic,
                SourceMirrorFailureKind::Transient => SourceMirrorErrorKind::Transient,
                SourceMirrorFailureKind::Timeout => SourceMirrorErrorKind::Timeout,
            },
            message: failure.message,
            attempts: failure.attempts,
            last_attempt_at: failure.last_attempt_at,
            next_attempt_at: failure.next_attempt_at,
        }
    }
}

#[derive(Deserialize, Debug, TS)]
pub struct ChangeTargetBranchRequest {
    pub repo_id: Uuid,
    pub new_target_branch: String,
}

#[derive(Serialize, Debug, TS)]
pub struct ChangeTargetBranchResponse {
    pub repo_id: Uuid,
    pub new_target_branch: String,
    pub status: (usize, usize),
}

#[derive(Deserialize, Debug, TS)]
pub struct RenameBranchRequest {
    pub new_branch_name: String,
}

#[derive(Serialize, Debug, TS)]
pub struct RenameBranchResponse {
    pub branch: String,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type", rename_all = "snake_case")]
pub enum RenameBranchError {
    EmptyBranchName,
    InvalidBranchNameFormat,
    OpenPullRequest,
    BranchAlreadyExists { repo_name: String },
    RebaseInProgress { repo_name: String },
    RenameFailed { repo_name: String, message: String },
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/status", get(get_workspace_branch_status))
        .route("/diff/ws", get(stream_diff_ws))
        .route("/merge", post(merge_workspace))
        .route("/push", post(push_workspace_branch))
        .route("/push/force", post(force_push_workspace_branch))
        .route("/rebase", post(rebase_workspace))
        .route("/rebase/continue", post(continue_workspace_rebase))
        .route("/conflicts/abort", post(abort_workspace_conflicts))
        .route("/target-branch", axum::routing::put(change_target_branch))
        .route("/branch", axum::routing::put(rename_branch))
}

async fn resolve_vibe_kanban_identifier(
    deployment: &DeploymentImpl,
    local_workspace_id: Uuid,
) -> String {
    if let Ok(client) = deployment.remote_client()
        && let Ok(remote_ws) = client.get_workspace_by_local_id(local_workspace_id).await
        && let Some(issue_id) = remote_ws.issue_id
        && let Ok(issue) = client.get_issue(issue_id).await
    {
        if !issue.simple_id.is_empty() {
            return issue.simple_id;
        }
        return issue_id.to_string();
    }
    local_workspace_id.to_string()
}

#[axum::debug_handler]
pub async fn stream_diff_ws(
    ws: SignedWsUpgrade,
    query: axum::extract::Query<DiffStreamQuery>,
    workspace: Extension<Workspace>,
    deployment: State<DeploymentImpl>,
) -> impl IntoResponse {
    stream_workspace_diff_ws(ws, query, workspace, deployment).await
}

#[axum::debug_handler]
pub async fn merge_workspace(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<MergeWorkspaceRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let pool = &deployment.db().pool;

    let workspace_repo =
        WorkspaceRepo::find_by_workspace_and_repo_id(pool, workspace.id, request.repo_id)
            .await?
            .ok_or(RepoError::NotFound)?;

    let repo = Repo::find_by_id(pool, workspace_repo.repo_id)
        .await?
        .ok_or(RepoError::NotFound)?;

    let merges = Merge::find_by_workspace_and_repo_id(pool, workspace.id, request.repo_id).await?;
    let has_open_pr = merges
        .iter()
        .any(|m| matches!(m, Merge::Pr(pr) if matches!(pr.pr_info.status, MergeStatus::Open)));
    if has_open_pr {
        return Err(ApiError::BadRequest(
            "Cannot merge directly when a pull request is open for this repository.".to_string(),
        ));
    }

    let is_target_remote = deployment
        .git()
        .is_remote_branch(&repo.path, &workspace_repo.target_branch)?;
    if is_target_remote {
        return Err(ApiError::BadRequest(
            "Cannot merge directly into a remote branch. Please create a pull request instead."
                .to_string(),
        ));
    }

    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_path = Path::new(&container_ref);
    let worktree_path = workspace_path.join(repo.name);

    let workspace_label = workspace.name.as_deref().unwrap_or(&workspace.branch);
    let vk_id = resolve_vibe_kanban_identifier(&deployment, workspace.id).await;
    let commit_message = format!("{} (vibe-kanban {})", workspace_label, vk_id);

    // Every await happens above this point: from the pinned-tip check below to
    // `merge_changes` there is none, so the verified tip is what gets merged
    // (as tight as a coordinator-local workspace, whose agent can also commit
    // concurrently with a merge).
    // `merge_changes` reads the workspace branch from the registered checkout.
    // For a clustered workspace that copy arrives through the background
    // mirror, so make sure it holds the worktree's HEAD before merging —
    // otherwise the merge could silently omit the latest commits.
    let (worktree_for_probe, checkout_for_probe) = (worktree_path.clone(), repo.path.clone());
    let identities = tokio::task::spawn_blocking(move || {
        let identity = |path: &Path| {
            let (common, _) = utils::worktree_linkage::common_dir_of(path)?;
            std::fs::canonicalize(&common)
                .map_err(|e| format!("canonicalize {}: {e}", common.display()))
        };
        Ok::<_, String>((
            identity(&worktree_for_probe)?,
            identity(&checkout_for_probe)?,
        ))
    });
    let (store, checkout_git) =
        match tokio::time::timeout(std::time::Duration::from_secs(10), identities).await {
            Ok(Ok(Ok(identities))) => identities,
            Ok(Ok(Err(e))) => {
                return Err(ApiError::BadRequest(format!(
                    "Cannot merge: could not identify the workspace's repository: {e}"
                )));
            }
            _ => {
                return Err(ApiError::BadRequest(
                    "Cannot merge yet: the repositories did not answer in time".to_string(),
                ));
            }
        };
    // For a clustered workspace, the exact commit verified below is what gets
    // merged (`merge_changes_pinned`), never a re-read of the branch name.
    let mut pinned_tip: Option<String> = None;
    if store != checkout_git {
        // The branch `merge_changes` merges, as the store has it — not the
        // worktree's HEAD, which may be detached or on another branch.
        let (store_dir, branch_ref) = (store.clone(), format!("refs/heads/{}", workspace.branch));
        let head = tokio::task::spawn_blocking(move || {
            git::GitCli::new().resolve_commit_within(
                &store_dir,
                &branch_ref,
                std::time::Duration::from_secs(10),
            )
        })
        .await
        .map_err(|e| ApiError::BadRequest(format!("Cannot merge yet: {e}")))?
        .map_err(|e| ApiError::BadRequest(format!("Cannot merge yet: {e}")))?
        .ok_or_else(|| {
            ApiError::BadRequest(format!(
                "Cannot merge: branch '{}' does not exist in the workspace's repository",
                workspace.branch
            ))
        })?;
        // The merge needs the pinned commit's *objects* in the checkout, not
        // its branch ref: the courtesy mirror's non-forced push cannot move a
        // ref that was rebased or amended, and that must not block a merge.
        // Fetch the commit into the checkout (FETCH_HEAD only, no ref is
        // touched; bounded), then confirm it is there.
        let (store_dir, checkout_dir, branch_ref, tip) = (
            store.clone(),
            repo.path.clone(),
            format!("refs/heads/{}", workspace.branch),
            head.clone(),
        );
        let fetched = tokio::task::spawn_blocking(move || {
            let cli = git::GitCli::new();
            let deadline = std::time::Duration::from_secs(120);
            let present = |cli: &git::GitCli| {
                cli.git_within(
                    &checkout_dir,
                    ["cat-file", "-e", &format!("{tip}^{{commit}}")],
                    None,
                    std::time::Duration::from_secs(10),
                )
                .is_ok()
            };
            if present(&cli) {
                return Ok(());
            }
            cli.git_within(
                &checkout_dir,
                [
                    "fetch",
                    "--no-tags",
                    "--no-write-fetch-head",
                    &store_dir.to_string_lossy(),
                    &branch_ref,
                ],
                None,
                deadline,
            )
            .map_err(|e| e.to_string())?;
            if present(&cli) {
                Ok(())
            } else {
                Err(format!("{tip} did not arrive in the registered checkout"))
            }
        })
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
        fetched.map_err(|e| {
            ApiError::BadRequest(format!(
                "Cannot merge yet: could not bring the workspace's commits into the registered \
                 checkout: {e}"
            ))
        })?;
        // Pinned: if the workspace moved on meanwhile, refuse instead of
        // merging a tip that omits the newer commits. Nothing is awaited
        // between this and `merge_changes_pinned`.
        let (store_dir, branch_ref) = (store.clone(), format!("refs/heads/{}", workspace.branch));
        let now = tokio::task::spawn_blocking(move || {
            git::GitCli::new()
                .resolve_commit_within(&store_dir, &branch_ref, std::time::Duration::from_secs(10))
                .ok()
                .flatten()
        })
        .await
        .unwrap_or(None);
        if now.as_deref() != Some(head.as_str()) {
            return Err(ApiError::BadRequest(
                "Cannot merge yet: the workspace received new commits while preparing the \
                 merge; try again"
                    .to_string(),
            ));
        }
        pinned_tip = Some(head);
    }

    let merge_commit_id = deployment.git().merge_changes_pinned(
        &repo.path,
        &worktree_path,
        &workspace.branch,
        &workspace_repo.target_branch,
        &commit_message,
        pinned_tip.as_deref(),
    )?;

    Merge::create_direct(
        pool,
        workspace.id,
        workspace_repo.repo_id,
        &workspace_repo.target_branch,
        &merge_commit_id,
    )
    .await?;

    // A clustered workspace's branch lives in the shared store: move it to the
    // squash commit too (compare-and-swap against the pinned tip, so a commit
    // that landed meanwhile is kept), exactly as `merge_changes` does for a
    // coordinator-local workspace's branch. Best effort, bounded, off-worker.
    if let Some(pinned) = pinned_tip.clone() {
        let (store_dir, checkout_dir, target_ref, branch_ref, squash) = (
            store.clone(),
            repo.path.clone(),
            format!("refs/heads/{}", workspace_repo.target_branch),
            format!("refs/heads/{}", workspace.branch),
            merge_commit_id.clone(),
        );
        // Detached: the merge is already recorded; this must finish (or
        // log) on its own even if the request goes away.
        let repo_display = repo.path.display().to_string();
        let branch_name = workspace.branch.clone();
        let handoff = tokio::spawn(async move {
            let moved = tokio::task::spawn_blocking(move || {
                let cli = git::GitCli::new();
                let deadline = std::time::Duration::from_secs(60);
                // The squash commit is the checkout target's tip: fetchable as a ref.
                cli.git_within(
                    &store_dir,
                    [
                        "fetch",
                        "--no-tags",
                        "--no-write-fetch-head",
                        &checkout_dir.to_string_lossy(),
                        &target_ref,
                    ],
                    None,
                    deadline,
                )?;
                cli.git_within(
                    &store_dir,
                    ["update-ref", &branch_ref, &squash, &pinned],
                    None,
                    deadline,
                )
            })
            .await;
            if !matches!(moved, Ok(Ok(_))) {
                tracing::warn!(
                    repo = %repo_display,
                    "merged {branch_name}, but could not move the workspace branch in the shared \
                     store to the squash commit: {moved:?}"
                );
            }
        });
        // Archive only after the hand-off (bounded): archiving first would let
        // a restore resume from the pre-squash branch.
        let _ = tokio::time::timeout(std::time::Duration::from_secs(130), handoff).await;
    }

    if let Ok(client) = deployment.remote_client() {
        let workspace_id = workspace.id;
        tokio::spawn(async move {
            remote_sync::sync_local_workspace_merge_to_remote(&client, workspace_id).await;
        });
    }

    Workspace::record_archive_intent(
        pool,
        workspace.id,
        &format!("merge:{merge_commit_id}"),
        "local_merge",
        chrono::Utc::now(),
    )
    .await?;
    if let Err(e) = deployment
        .container()
        .archive_workspace_automatically(workspace.id)
        .await
    {
        tracing::error!("Failed to archive workspace {}: {}", workspace.id, e);
    }

    deployment
        .track_if_analytics_allowed(
            "task_attempt_merged",
            serde_json::json!({
                "workspace_id": workspace.id.to_string(),
            }),
        )
        .await;

    Ok(ResponseJson(ApiResponse::success(())))
}

pub async fn push_workspace_branch(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<PushWorkspaceRequest>,
) -> Result<ResponseJson<ApiResponse<(), PushError>>, ApiError> {
    let pool = &deployment.db().pool;

    let workspace_repo =
        WorkspaceRepo::find_by_workspace_and_repo_id(pool, workspace.id, request.repo_id)
            .await?
            .ok_or(RepoError::NotFound)?;

    let repo = Repo::find_by_id(pool, workspace_repo.repo_id)
        .await?
        .ok_or(RepoError::NotFound)?;

    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_path = Path::new(&container_ref);
    let worktree_path = workspace_path.join(&repo.name);

    let credentials =
        github_credentials::resolve_for_repo(pool, deployment.git(), &repo.path).await;
    match deployment
        .git()
        .push_to_remote(&worktree_path, &workspace.branch, false, &credentials)
    {
        Ok(_) => {
            if let Ok(client) = deployment.remote_client() {
                let pool = deployment.db().pool.clone();
                let git = deployment.git().clone();
                let mut ws = workspace.clone();
                ws.container_ref = Some(container_ref.clone());
                tokio::spawn(async move {
                    let stats = diff_stream::compute_diff_stats(&pool, &git, &ws).await;
                    remote_sync::sync_workspace_to_remote(
                        &client,
                        ws.id,
                        None,
                        None,
                        stats.as_ref(),
                    )
                    .await;
                });
            }
            Ok(ResponseJson(ApiResponse::success(())))
        }
        Err(GitServiceError::GitCLI(GitCliError::PushRejected(_))) => Ok(ResponseJson(
            ApiResponse::error_with_data(PushError::ForcePushRequired),
        )),
        Err(e) => Err(ApiError::GitService(e)),
    }
}

pub async fn force_push_workspace_branch(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<PushWorkspaceRequest>,
) -> Result<ResponseJson<ApiResponse<(), PushError>>, ApiError> {
    let pool = &deployment.db().pool;

    let workspace_repo =
        WorkspaceRepo::find_by_workspace_and_repo_id(pool, workspace.id, request.repo_id)
            .await?
            .ok_or(RepoError::NotFound)?;

    let repo = Repo::find_by_id(pool, workspace_repo.repo_id)
        .await?
        .ok_or(RepoError::NotFound)?;

    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_path = Path::new(&container_ref);
    let worktree_path = workspace_path.join(&repo.name);

    let credentials =
        github_credentials::resolve_for_repo(pool, deployment.git(), &repo.path).await;
    deployment
        .git()
        .push_to_remote(&worktree_path, &workspace.branch, true, &credentials)?;

    if let Ok(client) = deployment.remote_client() {
        let pool = deployment.db().pool.clone();
        let git = deployment.git().clone();
        let mut ws = workspace.clone();
        ws.container_ref = Some(container_ref.clone());
        tokio::spawn(async move {
            let stats = diff_stream::compute_diff_stats(&pool, &git, &ws).await;
            remote_sync::sync_workspace_to_remote(&client, ws.id, None, None, stats.as_ref()).await;
        });
    }

    Ok(ResponseJson(ApiResponse::success(())))
}

pub async fn get_workspace_branch_status(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<RepoBranchStatus>>>, ApiError> {
    let pool = &deployment.db().pool;

    let repositories = WorkspaceRepo::find_repos_for_workspace(pool, workspace.id).await?;
    let workspace_repos = WorkspaceRepo::find_by_workspace_id(pool, workspace.id).await?;
    let target_branches: HashMap<_, _> = workspace_repos
        .iter()
        .map(|wr| (wr.repo_id, wr.target_branch.clone()))
        .collect();

    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_dir = PathBuf::from(&container_ref);

    let all_merges = Merge::find_by_workspace_id(pool, workspace.id).await?;
    let merges_by_repo: HashMap<Uuid, Vec<Merge>> =
        all_merges
            .into_iter()
            .fold(HashMap::new(), |mut acc, merge| {
                let repo_id = match &merge {
                    Merge::Direct(dm) => dm.repo_id,
                    Merge::Pr(pm) => pm.repo_id,
                };
                acc.entry(repo_id).or_insert_with(Vec::new).push(merge);
                acc
            });

    let mut results = Vec::with_capacity(repositories.len());

    for repo in repositories {
        let Some(target_branch) = target_branches.get(&repo.id).cloned() else {
            continue;
        };

        let repo_merges = merges_by_repo.get(&repo.id).cloned().unwrap_or_default();
        let worktree_path = workspace_dir.join(&repo.name);

        let head_oid = deployment
            .git()
            .get_head_info(&worktree_path)
            .ok()
            .map(|h| h.oid);

        let (is_rebase_in_progress, conflicted_files, conflict_op) = {
            let in_rebase = deployment
                .git()
                .is_rebase_in_progress(&worktree_path)
                .unwrap_or(false);
            let conflicts = deployment
                .git()
                .get_conflicted_files(&worktree_path)
                .unwrap_or_default();
            let op = if conflicts.is_empty() {
                None
            } else {
                deployment
                    .git()
                    .detect_conflict_op(&worktree_path)
                    .unwrap_or(None)
            };
            (in_rebase, conflicts, op)
        };

        let (uncommitted_count, untracked_count) =
            match deployment.git().get_worktree_change_counts(&worktree_path) {
                Ok((a, b)) => (Some(a), Some(b)),
                Err(_) => (None, None),
            };

        let has_uncommitted_changes = uncommitted_count.map(|c| c > 0);

        // The target's kind comes from the registered checkout — the same
        // local-first resolution merge and rebase use — with the worktree's
        // repository as the fallback; failing both, assume local rather than
        // failing the response and hiding `source_mirror_error` with it.
        let is_target_remote = deployment
            .git()
            .is_remote_branch(&repo.path, &target_branch)
            .or_else(|_| {
                deployment
                    .git()
                    .is_remote_branch(&worktree_path, &target_branch)
            })
            .unwrap_or(false);

        let has_open_pr = matches!(
            repo_merges.first(),
            Some(Merge::Pr(PrMerge {
                pr_info: PullRequestInfo {
                    status: MergeStatus::Open,
                    ..
                },
                ..
            }))
        );
        // Only fetching needs credentials; skip the lookup otherwise.
        let credentials = if is_target_remote || has_open_pr {
            github_credentials::resolve_for_repo(pool, deployment.git(), &repo.path).await
        } else {
            Default::default()
        };

        // Which repository to read. The registered checkout, exactly as
        // before, whenever it already holds the worktree's HEAD for this
        // branch: then its target branch is the current one (the same source
        // rebase uses). Otherwise — a clustered workspace whose background
        // mirror is pending, disabled or failing — the worktree's own
        // repository, the shared store, which has the real branch tip. The
        // other is the fallback; failing both, report unknown counts rather
        // than failing the response and hiding `source_mirror_error`.

        // A checkout that does not hold the worktree's HEAD is never read as a
        // fallback: its copy of the branch is stale and would under-report.
        // The workspace side is always read from the worktree's own
        // repository — the shared store, for a clustered workspace — never
        // from the checkout's courtesy copy, which may lag or have been moved.
        // (A coordinator-local worktree shares its checkout's refs anyway.)
        let (primary, secondary): (&std::path::Path, Option<&std::path::Path>) =
            (&worktree_path, None);
        let branch_status_in = |repo_path: &std::path::Path| {
            if is_target_remote {
                deployment.git().get_remote_branch_status(
                    repo_path,
                    &workspace.branch,
                    Some(&target_branch),
                    &credentials,
                )
            } else {
                deployment
                    .git()
                    .get_branch_status(repo_path, &workspace.branch, &target_branch)
            }
        };
        // A local target is compared as the checkout has it *now* (the same
        // source direct merge and rebase use), against the worktree's HEAD.
        // Off the runtime workers. When it cannot compare against the current
        // target (e.g. the store lacks that commit yet), the counts are
        // unknown — never a comparison against an older copy of the target.
        let counts = if is_target_remote {
            let via_git = branch_status_in(primary)
                .or_else(|e| secondary.map_or(Err(e), &branch_status_in))
                .map_err(|e| e.to_string());
            match via_git {
                Ok(counts) => Ok(counts),
                // E.g. `upstream/main`, whose remote the store has no fetch
                // mapping for: compare the workspace branch against the
                // checkout's remote-tracking ref instead (bounded, off-worker).
                Err(_) => {
                    // Refresh the target in the checkout first (the same
                    // credentialed fetch this status always did there),
                    // whether or not its copy of the workspace branch is
                    // current; its own counts are ignored.
                    // Comparing the target with itself: a fetch of its remote
                    // through the usual credentialed path that needs nothing
                    // but the target ref (counts ignored).
                    let refreshed = deployment.git().get_remote_branch_status(
                        &repo.path,
                        &target_branch,
                        Some(&target_branch),
                        &credentials,
                    );
                    match refreshed {
                        // Unknown rather than a comparison against a cached,
                        // possibly stale tracking ref.
                        Err(e) => Err(format!("could not refresh '{target_branch}': {e}")),
                        Ok(_) => {
                            let (worktree, checkout, branch, target) = (
                                worktree_path.clone(),
                                repo.path.clone(),
                                workspace.branch.clone(),
                                target_branch.clone(),
                            );
                            tokio::task::spawn_blocking(move || {
                                counts_against_checkout_target(
                                    &worktree, &checkout, &branch, &target,
                                )
                            })
                            .await
                            .unwrap_or_else(|e| Err(e.to_string()))
                        }
                    }
                }
            }
        } else {
            let (worktree, checkout, branch, target) = (
                worktree_path.clone(),
                repo.path.clone(),
                workspace.branch.clone(),
                target_branch.clone(),
            );
            tokio::task::spawn_blocking(move || {
                // Bounded throughout; it brings a missing target commit in.
                counts_against_checkout_target(&worktree, &checkout, &branch, &target)
            })
            .await
            .unwrap_or_else(|e| Err(e.to_string()))
        };
        let (commits_ahead, commits_behind) = match counts {
            Ok((ahead, behind)) => (Some(ahead), Some(behind)),
            Err(e) => {
                tracing::debug!(
                    repo = %repo.name,
                    "branch status unavailable for '{}': {e}",
                    workspace.branch
                );
                (None, None)
            }
        };

        let (remote_ahead, remote_behind) = if has_open_pr {
            // Worktree first, for the same reason as above.
            let pr_status_in = |repo_path: &std::path::Path| {
                deployment.git().get_remote_branch_status(
                    repo_path,
                    &workspace.branch,
                    None,
                    &credentials,
                )
            };
            match pr_status_in(primary).or_else(|e| secondary.map_or(Err(e), &pr_status_in)) {
                Ok((ahead, behind)) => (Some(ahead), Some(behind)),
                // The branch's upstream may be configured only in the
                // checkout (a PR opened before the workspace moved to the
                // store): resolve it there, compare it with the store's branch.
                Err(_) => {
                    // Refresh the checkout's remote first (the same
                    // credentialed fetch this status always did there); its
                    // own counts may rest on a stale workspace copy, so they
                    // are ignored in favour of the store's branch below.
                    let refreshed = deployment
                        .git()
                        .get_remote_branch_status(&repo.path, &workspace.branch, None, &credentials)
                        .is_ok();
                    let (worktree, checkout, branch) = (
                        worktree_path.clone(),
                        repo.path.clone(),
                        workspace.branch.clone(),
                    );
                    let counts = tokio::task::spawn_blocking(move || {
                        let upstream = git::GitCli::new()
                            .git_within(
                                &checkout,
                                [
                                    "rev-parse",
                                    "--abbrev-ref",
                                    "--symbolic-full-name",
                                    &format!("{branch}@{{upstream}}"),
                                ],
                                None,
                                std::time::Duration::from_secs(5),
                            )
                            .map_err(|e| e.to_string())?;
                        counts_against_checkout_target(
                            &worktree,
                            &checkout,
                            &branch,
                            upstream.trim(),
                        )
                    })
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()));
                    match counts {
                        // Only against a freshly fetched upstream.
                        Ok((ahead, behind)) if refreshed => (Some(ahead), Some(behind)),
                        _ => (None, None),
                    }
                }
            }
        } else {
            (None, None)
        };

        let source_mirror_error = SourceMirror::global()
            .failure(&repo.path, &workspace.branch)
            .map(SourceMirrorError::from);

        results.push(RepoBranchStatus {
            repo_id: repo.id,
            repo_name: repo.name,
            source_mirror_error,
            status: BranchStatus {
                commits_ahead,
                commits_behind,
                has_uncommitted_changes,
                head_oid,
                uncommitted_count,
                untracked_count,
                remote_commits_ahead: remote_ahead,
                remote_commits_behind: remote_behind,
                merges: repo_merges,
                target_branch_name: target_branch,
                is_rebase_in_progress,
                conflict_op,
                conflicted_files,
                is_target_remote,
            },
        });
    }

    Ok(ResponseJson(ApiResponse::success(results)))
}

/// Commits ahead of / behind `target` *as the registered checkout has it now*,
/// for the workspace branch in the worktree's own repository — which, for a
/// clustered workspace, is the
/// authoritative workspace tip even while its courtesy mirror into the
/// checkout is pending, disabled or failing. Computed in the worktree's
/// repository; fails if the checkout's target commit is not present there.
/// Background target fetches (see `counts_against_checkout_target`), keyed by
/// (repository, target ref).
static TARGET_FETCHES: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<(PathBuf, String), TargetFetch>>,
> = std::sync::LazyLock::new(Default::default);
const TARGET_FETCH_BACKOFF: std::time::Duration = std::time::Duration::from_secs(60);

enum TargetFetch {
    Running,
    FailedAt(std::time::Instant),
}

fn counts_against_checkout_target(
    worktree: &std::path::Path,
    checkout: &std::path::Path,
    branch: &str,
    target: &str,
) -> Result<(usize, usize), String> {
    // One stuck probe per stalled checkout at most: while an earlier one has
    // not exited, report unknown rather than start another.
    if git::GitCli::has_stuck_command(checkout) || git::GitCli::has_stuck_command(worktree) {
        return Err(format!(
            "an earlier read of {} has not finished (stalled storage?)",
            checkout.display()
        ));
    }
    // Every call under a process-level deadline: the checkout may be stalled.
    let deadline = std::time::Duration::from_secs(5);
    let cli = git::GitCli::new();
    // As a branch — local first, then remote-tracking — the way
    // `GitService::find_branch` resolves it; never a same-named tag.
    let mut found = None;
    for candidate in [
        format!("refs/heads/{target}"),
        format!("refs/remotes/{target}"),
    ] {
        if let Some(oid) = cli
            .resolve_commit_within(checkout, &candidate, deadline)
            .map_err(|e| e.to_string())?
        {
            found = Some((candidate, oid));
            break;
        }
    }
    let (target_ref, tip) =
        found.ok_or_else(|| format!("'{target}' was not found in {}", checkout.display()))?;
    let present = |cli: &git::GitCli| {
        cli.git_within(
            worktree,
            ["cat-file", "-e", &format!("{tip}^{{commit}}")],
            None,
            deadline,
        )
        .is_ok()
    };
    if !present(&cli) {
        // The workspace's repository (the shared store, for a clustered
        // workspace) lacks the checkout's current target commit. Fetch just
        // that ref's objects (FETCH_HEAD only, no ref moves; bounded) in the
        // background, at most once at a time per (repository, ref) however
        // many polls ask; counts are unknown until it lands.
        // Keyed by the repository itself (many worktrees share one store),
        // with a back-off after a failed fetch.
        let repository = utils::worktree_linkage::common_dir_of(worktree)
            .ok()
            .and_then(|(common, _)| std::fs::canonicalize(common).ok())
            .unwrap_or_else(|| worktree.to_path_buf());
        let key = (repository.clone(), target_ref.clone());
        if git::GitCli::has_stuck_command(&repository) {
            return Err(format!(
                "an earlier fetch into {} has not exited (stalled storage?)",
                repository.display()
            ));
        }
        let started = {
            let mut fetches = TARGET_FETCHES.lock().unwrap_or_else(|e| e.into_inner());
            match fetches.get(&key) {
                Some(TargetFetch::Running) => false,
                Some(TargetFetch::FailedAt(at)) if at.elapsed() < TARGET_FETCH_BACKOFF => false,
                _ => {
                    fetches.insert(key.clone(), TargetFetch::Running);
                    true
                }
            }
        };
        if started {
            let checkout = checkout.to_path_buf();
            std::thread::spawn(move || {
                let fetched = git::GitCli::new().git_within(
                    &repository,
                    [
                        "fetch",
                        "--no-tags",
                        "--no-write-fetch-head",
                        &checkout.to_string_lossy(),
                        &key.1,
                    ],
                    None,
                    std::time::Duration::from_secs(120),
                );
                let mut fetches = TARGET_FETCHES.lock().unwrap_or_else(|e| e.into_inner());
                match fetched {
                    Ok(_) => {
                        fetches.remove(&key);
                    }
                    Err(_) => {
                        fetches.insert(key, TargetFetch::FailedAt(std::time::Instant::now()));
                    }
                }
            });
        }
        return Err(format!(
            "{tip} is not in the workspace's repository yet; fetching it"
        ));
    }
    let out = cli
        .git_within(
            worktree,
            [
                "rev-list",
                "--left-right",
                "--count",
                &format!("refs/heads/{branch}...{tip}"),
            ],
            None,
            deadline,
        )
        .map_err(|e| e.to_string())?;
    let mut counts = out.split_whitespace().map(str::parse::<usize>);
    match (counts.next(), counts.next()) {
        (Some(Ok(ahead)), Some(Ok(behind))) => Ok((ahead, behind)),
        _ => Err(format!("unexpected rev-list output: {out}")),
    }
}

#[axum::debug_handler]
pub async fn change_target_branch(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<ChangeTargetBranchRequest>,
) -> Result<ResponseJson<ApiResponse<ChangeTargetBranchResponse>>, ApiError> {
    let repo_id = payload.repo_id;
    let new_target_branch = payload.new_target_branch;
    let pool = &deployment.db().pool;

    let repo = Repo::find_by_id(pool, repo_id)
        .await?
        .ok_or(RepoError::NotFound)?;

    if !deployment
        .git()
        .check_branch_exists(&repo.path, &new_target_branch)?
    {
        return Ok(ResponseJson(ApiResponse::error(
            format!(
                "Branch '{}' does not exist in repository '{}'",
                new_target_branch, repo.name
            )
            .as_str(),
        )));
    };

    // Validate before persisting. The worktree's HEAD is authoritative (for a
    // clustered workspace the checkout's copy of the branch may lag or be
    // missing); the checkout's own comparison is the fallback.
    let worktree = workspace
        .container_ref
        .as_ref()
        .filter(|_| !workspace.worktree_deleted)
        .map(|dir| PathBuf::from(dir).join(&repo.name));
    let status = match worktree {
        // Off the runtime workers, every git call under a deadline; a failure
        // here is reported, never retried against an unbounded checkout read.
        Some(worktree) => {
            // The authoritative tip of the workspace branch (the worktree's
            // own repository — the shared store for a clustered workspace).
            let worktree_tip = {
                let (worktree, branch_ref) =
                    (worktree.clone(), format!("refs/heads/{}", workspace.branch));
                tokio::task::spawn_blocking(move || {
                    git::GitCli::new()
                        .resolve_commit_within(
                            &worktree,
                            &branch_ref,
                            std::time::Duration::from_secs(5),
                        )
                        .ok()
                        .flatten()
                })
                .await
                .unwrap_or(None)
            };
            let (checkout, branch, target) = (
                repo.path.clone(),
                workspace.branch.clone(),
                new_target_branch.clone(),
            );
            let counts = tokio::task::spawn_blocking(move || {
                // A mutation: give a background fetch of a target the store
                // lacks a bounded moment to land, then compare again.
                let mut result =
                    counts_against_checkout_target(&worktree, &checkout, &branch, &target);
                for _ in 0..30 {
                    match &result {
                        Err(e) if e.ends_with("fetching it") => {
                            std::thread::sleep(std::time::Duration::from_secs(1));
                            result = counts_against_checkout_target(
                                &worktree, &checkout, &branch, &target,
                            );
                        }
                        _ => break,
                    }
                }
                result
            })
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
            match counts {
                Ok(status) => status,
                // E.g. a new target whose tip the store has not got yet: the
                // checkout can still compare its own refs (off-worker,
                // bounded wait).
                Err(first) => {
                    let (checkout, branch, target) = (
                        repo.path.clone(),
                        workspace.branch.clone(),
                        new_target_branch.clone(),
                    );
                    let authoritative = worktree_tip.clone();
                    let fallback = tokio::task::spawn_blocking(move || {
                        // Only if the checkout's copy of the workspace branch
                        // is verifiably the authoritative tip; otherwise its
                        // counts would describe an older commit.
                        let checkout_tip = git::GitCli::new()
                            .resolve_commit_within(
                                &checkout,
                                &format!("refs/heads/{branch}"),
                                std::time::Duration::from_secs(5),
                            )
                            .ok()
                            .flatten();
                        if authoritative.is_none() || checkout_tip != authoritative {
                            return Err("the registered checkout's copy of the workspace \
                                        branch is not current"
                                .to_string());
                        }
                        git::GitService::new()
                            .get_branch_status(&checkout, &branch, &target)
                            .map_err(|e| e.to_string())
                    });
                    match tokio::time::timeout(std::time::Duration::from_secs(10), fallback).await {
                        Ok(Ok(Ok(status))) => status,
                        _ => {
                            return Ok(ResponseJson(ApiResponse::error(
                                format!(
                                    "Could not compare the workspace with \
                                     '{new_target_branch}': {first}"
                                )
                                .as_str(),
                            )));
                        }
                    }
                }
            }
        }
        None => {
            deployment
                .git()
                .get_branch_status(&repo.path, &workspace.branch, &new_target_branch)?
        }
    };

    WorkspaceRepo::update_target_branch(pool, workspace.id, repo_id, &new_target_branch).await?;

    deployment
        .track_if_analytics_allowed(
            "task_attempt_target_branch_changed",
            serde_json::json!({
                "repo_id": repo_id.to_string(),
                "workspace_id": workspace.id.to_string(),
            }),
        )
        .await;

    Ok(ResponseJson(ApiResponse::success(
        ChangeTargetBranchResponse {
            repo_id,
            new_target_branch,
            status,
        },
    )))
}

#[axum::debug_handler]
pub async fn rename_branch(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<RenameBranchRequest>,
) -> Result<ResponseJson<ApiResponse<RenameBranchResponse, RenameBranchError>>, ApiError> {
    let new_branch_name = payload.new_branch_name.trim();

    if new_branch_name.is_empty() {
        return Ok(ResponseJson(ApiResponse::error_with_data(
            RenameBranchError::EmptyBranchName,
        )));
    }
    if !deployment.git().is_branch_name_valid(new_branch_name) {
        return Ok(ResponseJson(ApiResponse::error_with_data(
            RenameBranchError::InvalidBranchNameFormat,
        )));
    }
    if new_branch_name == workspace.branch {
        return Ok(ResponseJson(ApiResponse::success(RenameBranchResponse {
            branch: workspace.branch.clone(),
        })));
    }

    let pool = &deployment.db().pool;

    let merges = Merge::find_by_workspace_id(pool, workspace.id).await?;
    let has_open_pr = merges.into_iter().any(|merge| {
        matches!(merge, Merge::Pr(pr_merge) if matches!(pr_merge.pr_info.status, MergeStatus::Open))
    });
    if has_open_pr {
        return Ok(ResponseJson(ApiResponse::error_with_data(
            RenameBranchError::OpenPullRequest,
        )));
    }

    let repos = WorkspaceRepo::find_repos_for_workspace(pool, workspace.id).await?;
    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_dir = PathBuf::from(&container_ref);

    for repo in &repos {
        let worktree_path = workspace_dir.join(&repo.name);

        if deployment
            .git()
            .check_branch_exists(&repo.path, new_branch_name)?
        {
            return Ok(ResponseJson(ApiResponse::error_with_data(
                RenameBranchError::BranchAlreadyExists {
                    repo_name: repo.name.clone(),
                },
            )));
        }

        if deployment.git().is_rebase_in_progress(&worktree_path)? {
            return Ok(ResponseJson(ApiResponse::error_with_data(
                RenameBranchError::RebaseInProgress {
                    repo_name: repo.name.clone(),
                },
            )));
        }
    }

    let old_branch = workspace.branch.clone();
    let mut renamed_repos: Vec<&Repo> = Vec::new();

    for repo in &repos {
        let worktree_path = workspace_dir.join(&repo.name);

        match deployment.git().rename_local_branch(
            &worktree_path,
            &workspace.branch,
            new_branch_name,
        ) {
            Ok(()) => {
                renamed_repos.push(repo);
            }
            Err(e) => {
                for renamed_repo in &renamed_repos {
                    let rollback_path = workspace_dir.join(&renamed_repo.name);
                    if let Err(rollback_err) = deployment.git().rename_local_branch(
                        &rollback_path,
                        new_branch_name,
                        &old_branch,
                    ) {
                        tracing::error!(
                            "Failed to rollback branch rename in '{}': {}",
                            renamed_repo.name,
                            rollback_err
                        );
                    }
                }
                return Ok(ResponseJson(ApiResponse::error_with_data(
                    RenameBranchError::RenameFailed {
                        repo_name: repo.name.clone(),
                        message: e.to_string(),
                    },
                )));
            }
        }
    }

    db::models::workspace::Workspace::update_branch_name(pool, workspace.id, new_branch_name)
        .await?;
    let updated_children_count = WorkspaceRepo::update_target_branch_for_children_of_workspace(
        pool,
        workspace.id,
        &old_branch,
        new_branch_name,
    )
    .await?;

    if updated_children_count > 0 {
        tracing::info!(
            "Updated {} child workspaces to target new branch '{}'",
            updated_children_count,
            new_branch_name
        );
    }

    deployment
        .track_if_analytics_allowed(
            "task_attempt_branch_renamed",
            serde_json::json!({
                "updated_children": updated_children_count,
            }),
        )
        .await;

    Ok(ResponseJson(ApiResponse::success(RenameBranchResponse {
        branch: new_branch_name.to_string(),
    })))
}

#[axum::debug_handler]
pub async fn rebase_workspace(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<RebaseWorkspaceRequest>,
) -> Result<ResponseJson<ApiResponse<(), GitOperationError>>, ApiError> {
    let pool = &deployment.db().pool;

    let workspace_repo =
        WorkspaceRepo::find_by_workspace_and_repo_id(pool, workspace.id, payload.repo_id)
            .await?
            .ok_or(RepoError::NotFound)?;

    let repo = Repo::find_by_id(pool, workspace_repo.repo_id)
        .await?
        .ok_or(RepoError::NotFound)?;

    let old_base_branch = payload
        .old_base_branch
        .unwrap_or_else(|| workspace_repo.target_branch.clone());
    let new_base_branch = payload
        .new_base_branch
        .unwrap_or_else(|| workspace_repo.target_branch.clone());

    match deployment
        .git()
        .check_branch_exists(&repo.path, &new_base_branch)?
    {
        true => {
            WorkspaceRepo::update_target_branch(
                pool,
                workspace.id,
                payload.repo_id,
                &new_base_branch,
            )
            .await?;
        }
        false => {
            return Ok(ResponseJson(ApiResponse::error(
                format!(
                    "Branch '{}' does not exist in the repository",
                    new_base_branch
                )
                .as_str(),
            )));
        }
    }

    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_path = Path::new(&container_ref);
    let worktree_path = workspace_path.join(&repo.name);

    let credentials =
        github_credentials::resolve_for_repo(pool, deployment.git(), &repo.path).await;
    let result = deployment.git().rebase_branch(
        &repo.path,
        &worktree_path,
        &new_base_branch,
        &old_base_branch,
        &workspace.branch.clone(),
        &credentials,
    );
    if let Err(e) = result {
        return match e {
            GitServiceError::MergeConflicts {
                message,
                conflicted_files,
            } => Ok(ResponseJson(
                ApiResponse::<(), GitOperationError>::error_with_data(
                    GitOperationError::MergeConflicts {
                        message,
                        op: ConflictOp::Rebase,
                        conflicted_files,
                        target_branch: new_base_branch.clone(),
                    },
                ),
            )),
            GitServiceError::RebaseInProgress => Ok(ResponseJson(ApiResponse::<
                (),
                GitOperationError,
            >::error_with_data(
                GitOperationError::RebaseInProgress,
            ))),
            other => Err(ApiError::GitService(other)),
        };
    }

    deployment
        .track_if_analytics_allowed(
            "task_attempt_rebased",
            serde_json::json!({
                "workspace_id": workspace.id.to_string(),
                "repo_id": payload.repo_id.to_string(),
            }),
        )
        .await;

    Ok(ResponseJson(ApiResponse::success(())))
}

#[axum::debug_handler]
pub async fn abort_workspace_conflicts(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<AbortConflictsRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let pool = &deployment.db().pool;

    let repo = Repo::find_by_id(pool, payload.repo_id)
        .await?
        .ok_or(RepoError::NotFound)?;

    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_path = Path::new(&container_ref);
    let worktree_path = workspace_path.join(&repo.name);

    deployment.git().abort_conflicts(&worktree_path)?;

    Ok(ResponseJson(ApiResponse::success(())))
}

#[axum::debug_handler]
pub async fn continue_workspace_rebase(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<ContinueRebaseRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let pool = &deployment.db().pool;

    let repo = Repo::find_by_id(pool, payload.repo_id)
        .await?
        .ok_or(RepoError::NotFound)?;

    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_path = Path::new(&container_ref);
    let worktree_path = workspace_path.join(&repo.name);

    deployment.git().continue_rebase(&worktree_path)?;

    Ok(ResponseJson(ApiResponse::success(())))
}
