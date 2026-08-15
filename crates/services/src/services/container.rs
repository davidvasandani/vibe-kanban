use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc, LazyLock, Mutex as StdMutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use anyhow::{Error as AnyhowError, anyhow};
use async_trait::async_trait;
use db::{
    DBService,
    models::{
        coding_agent_turn::{CodingAgentTurn, CreateCodingAgentTurn},
        execution_process::{
            CreateExecutionProcess, ExecutionContext, ExecutionProcess, ExecutionProcessError,
            ExecutionProcessRunReason, ExecutionProcessStatus,
        },
        execution_process_repo_state::{
            CreateExecutionProcessRepoState, ExecutionProcessRepoState,
        },
        execution_worker_job::ExecutionWorkerJob,
        repo::Repo,
        session::{CreateSession, Session, SessionError},
        workspace::{Workspace, WorkspaceError},
        workspace_repo::WorkspaceRepo,
    },
};
#[cfg(feature = "qa-mode")]
use executors::executors::qa_mock::QaMockExecutor;
#[cfg(not(feature = "qa-mode"))]
use executors::profile::ExecutorConfigs;
use executors::{
    actions::{
        ExecutorAction, ExecutorActionType,
        coding_agent_follow_up::CodingAgentFollowUpRequest,
        coding_agent_initial::CodingAgentInitialRequest,
        script::{ScriptContext, ScriptRequest, ScriptRequestLanguage},
    },
    executors::{ExecutorError, StandardCodingAgentExecutor},
    logs::{
        NormalizedEntry, NormalizedEntryError, NormalizedEntryType,
        utils::{
            ConversationPatch,
            patch::{fix_patch_ops, is_add_or_replace, patch_entry_path},
        },
    },
    mcp_refresh::McpRefreshResult,
    profile::{ExecutorConfig, ExecutorProfileId},
};
use futures::{StreamExt, future, stream::BoxStream};
use git::{GitService, GitServiceError};
use json_patch::Patch;
use sqlx::Error as SqlxError;
use thiserror::Error;
use tokio::{
    sync::{Mutex, OwnedMutexGuard, OwnedSemaphorePermit, RwLock, Semaphore},
    task::{AbortHandle, JoinHandle},
};
use utils::{
    log_msg::LogMsg,
    msg_store::MsgStore,
    text::{git_branch_id, short_uuid},
};
use uuid::Uuid;
use worktree_manager::WorktreeError;

use crate::services::{execution_process, normalized_log_cache, notification::NotificationService};

/// Store the settled entries a historical replay produced, so the next reader
/// replays them instead of re-normalizing the raw log.
///
/// Best-effort throughout: this is a cache over a source of truth that is still
/// there. A failure to materialize costs the next reader time, which is the
/// situation before this existed — so it is logged and dropped rather than
/// failing a read that has already succeeded.
async fn materialize_normalized_log(
    cache_path: &std::path::Path,
    patches: &[Patch],
    truncated: bool,
) -> bool {
    if patches.is_empty() {
        return false;
    }

    let entries = match normalized_log_cache::materialize_entries(patches) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!("Could not materialize normalized log: {e}");
            return false;
        }
    };

    let header = normalized_log_cache::CacheHeader {
        version: normalized_log_cache::CACHE_VERSION,
        entry_count: entries.len(),
        truncated,
    };
    if let Err(e) = normalized_log_cache::write(cache_path, header, &entries).await {
        tracing::warn!("Could not store materialized normalized log: {e}");
        return false;
    }
    true
}

async fn replay_materialized_log(
    cache_path: &Path,
    execution_id: Uuid,
    cache_stage: &'static str,
) -> Option<BoxStream<'static, Result<LogMsg, std::io::Error>>> {
    let (_, entries) = normalized_log_cache::read(cache_path).await?;
    match normalized_log_cache::entries_as_patches(&entries) {
        Ok(patches) => {
            tracing::info!(
                execution_id = %execution_id,
                entry_count = patches.len(),
                cache_stage,
                "Serving normalized log from its materialized view"
            );
            Some(
                futures::stream::iter(patches)
                    .map(|patch| Ok::<_, std::io::Error>(LogMsg::JsonPatch(patch)))
                    .chain(futures::stream::once(async {
                        Ok::<_, std::io::Error>(LogMsg::Finished)
                    }))
                    .boxed(),
            )
        }
        Err(e) => {
            tracing::warn!(
                execution_id = %execution_id,
                cache_stage,
                "Materialized normalized log could not be replayed, re-deriving it: {e}"
            );
            None
        }
    }
}

/// True for a patch targeting `/entries/<numeric index>` — a conversation
/// entry, as opposed to a repo-diff patch (`/entries/<repo>/<file>`), which
/// targets a nested object `materialize_entries` can't apply against the
/// `{"entries": []}` array document.
fn is_indexed_entry_patch(patch: &Patch) -> bool {
    patch_entry_path(patch)
        .and_then(|path| path.strip_prefix("/entries/").map(str::to_string))
        .is_some_and(|rest| rest.parse::<usize>().is_ok())
}

pub type ContainerRef = String;

// Historical normalization temporarily holds raw logs, parsed messages, and
// normalized patches. Serialize it so reconnects or multiple browser tabs
// cannot multiply that memory inside the server process.
static HISTORICAL_NORMALIZATION_PERMITS: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(1)));

struct HistoricalNormalizationRegistry {
    cells: Arc<StdMutex<HashMap<Uuid, Weak<Mutex<()>>>>>,
}

struct HistoricalNormalizationLease {
    _guard: OwnedMutexGuard<()>,
    cells: Arc<StdMutex<HashMap<Uuid, Weak<Mutex<()>>>>>,
    execution_id: Uuid,
    cell: Weak<Mutex<()>>,
    joined_existing: bool,
    wait_started: Instant,
}

impl Default for HistoricalNormalizationRegistry {
    fn default() -> Self {
        Self {
            cells: Arc::new(StdMutex::new(HashMap::new())),
        }
    }
}

impl HistoricalNormalizationRegistry {
    async fn acquire(&self, execution_id: Uuid) -> HistoricalNormalizationLease {
        let wait_started = Instant::now();
        let (cell, joined_existing) = {
            let mut cells = self.cells.lock().unwrap_or_else(|e| e.into_inner());
            // Weak values mean completed executions do not accumulate forever.
            // Retaining here leaves at most the active cells plus this request's
            // cell, without needing async cleanup from a Drop implementation.
            cells.retain(|_, cell| cell.strong_count() > 0);
            match cells.get(&execution_id).and_then(Weak::upgrade) {
                Some(cell) => (cell, true),
                None => {
                    let cell = Arc::new(Mutex::new(()));
                    cells.insert(execution_id, Arc::downgrade(&cell));
                    (cell, false)
                }
            }
        };
        let cell_weak = Arc::downgrade(&cell);
        let guard = cell.lock_owned().await;
        HistoricalNormalizationLease {
            _guard: guard,
            cells: self.cells.clone(),
            execution_id,
            cell: cell_weak,
            joined_existing,
            wait_started,
        }
    }
}

impl Drop for HistoricalNormalizationLease {
    fn drop(&mut self) {
        // The owned mutex guard is the cell's final strong reference when no
        // waiter exists. Remove only our exact generation: a new request may
        // already have installed a replacement cell for this execution ID.
        if self.cell.strong_count() != 1 {
            return;
        }
        let mut cells = self.cells.lock().unwrap_or_else(|e| e.into_inner());
        if cells
            .get(&self.execution_id)
            .is_some_and(|cell| Weak::ptr_eq(cell, &self.cell))
        {
            cells.remove(&self.execution_id);
        }
    }
}

static HISTORICAL_NORMALIZATIONS: LazyLock<HistoricalNormalizationRegistry> =
    LazyLock::new(HistoricalNormalizationRegistry::default);

struct HistoricalNormalizationLifetime {
    _permit: OwnedSemaphorePermit,
    _lease: HistoricalNormalizationLease,
    tasks: Vec<AbortHandle>,
    execution_id: Uuid,
    completed: Arc<AtomicBool>,
}

impl Drop for HistoricalNormalizationLifetime {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
        if !self.completed.load(Ordering::Relaxed) {
            tracing::info!(
                execution_id = %self.execution_id,
                "Historical log normalization canceled before materialization completed"
            );
        }
    }
}

/// Follow-up prompt sent when resuming a coding-agent run that was
/// interrupted by a server shutdown/restart. Keep in sync with
/// RESUME_INTERRUPTED_PROMPT in SessionChatBoxContainer.tsx (the Resume
/// banner) so manual and automatic resumes read the same in the session
/// history. Auto-resume also uses it to recognize runs it already resumed
/// once (see [`executor_config_for_auto_resume`]).
pub const RESUME_INTERRUPTED_PROMPT: &str = "The previous run was interrupted by a vibe-kanban restart before it could finish. Review the current state of the working tree and continue the task from where it left off.";

fn reset_would_discard_uncommitted_work(
    perform_git_reset: bool,
    is_dirty: bool,
    force_when_dirty: bool,
) -> bool {
    perform_git_reset && is_dirty && !force_when_dirty
}

/// Decide whether an interrupted coding-agent action is eligible for
/// auto-resume, and with which executor config. Returns `None` when the
/// action is not a coding-agent request, or when it is itself a resume
/// follow-up — resuming those again would spawn agents unattended on every
/// boot of a crash-restart loop, so each run is resumed at most once.
fn executor_config_for_auto_resume(action: &ExecutorAction) -> Option<ExecutorConfig> {
    match action.typ() {
        ExecutorActionType::CodingAgentInitialRequest(request) => {
            Some(request.executor_config.clone())
        }
        ExecutorActionType::CodingAgentFollowUpRequest(request)
            if request.prompt != RESUME_INTERRUPTED_PROMPT =>
        {
            Some(request.executor_config.clone())
        }
        _ => None,
    }
}

#[derive(Debug, Error)]
pub enum ContainerError {
    #[error(transparent)]
    GitServiceError(#[from] GitServiceError),
    #[error(transparent)]
    Sqlx(#[from] SqlxError),
    #[error(transparent)]
    ExecutorError(#[from] ExecutorError),
    #[error(transparent)]
    Worktree(#[from] WorktreeError),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(transparent)]
    ExecutionProcess(#[from] ExecutionProcessError),
    #[error("Io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to kill process: {0}")]
    KillFailed(std::io::Error),
    /// A clustered workspace could not be provisioned from its shared
    /// repository store. Distinct from `Other` so the message survives to the
    /// API instead of being rendered as a generic internal error.
    #[error("{0}")]
    SharedStore(String),
    #[error(transparent)]
    Other(#[from] AnyhowError), // Catches any unclassified errors
}

#[async_trait]
pub trait ContainerService {
    fn msg_stores(&self) -> &Arc<RwLock<HashMap<Uuid, Arc<MsgStore>>>>;

    fn db(&self) -> &DBService;

    fn git(&self) -> &GitService;

    fn notification_service(&self) -> &NotificationService;

    async fn refresh_mcp_tools(
        &self,
        workspace_id: Uuid,
        session_id: Uuid,
    ) -> Result<McpRefreshResult, ContainerError>;

    async fn mcp_refresh_status(
        &self,
        workspace_id: Uuid,
        session_id: Uuid,
    ) -> Result<Option<McpRefreshResult>, ContainerError>;

    /// Resolve the owning organization's environment variables for a
    /// workspace. Implementations must degrade to an empty map when the
    /// workspace is local-only or the remote configuration is unavailable.
    async fn resolve_org_env_vars(&self, workspace: &Workspace) -> HashMap<String, String>;

    async fn touch(&self, workspace: &Workspace) -> Result<(), ContainerError>;

    fn workspace_to_current_dir(&self, workspace: &Workspace) -> PathBuf;

    async fn discover_executor_options(
        &self,
        executor_profile_id: ExecutorProfileId,
        session_id: Option<Uuid>,
        workspace_id: Option<Uuid>,
        repo_id: Option<Uuid>,
    ) -> Result<Option<BoxStream<'static, Patch>>, ContainerError> {
        let (workdir, repo_path) = if let Some(session_id) = session_id {
            let session = Session::find_by_id(&self.db().pool, session_id)
                .await?
                .ok_or(SqlxError::RowNotFound)?;

            if let Some(workspace_id) = workspace_id
                && session.workspace_id != workspace_id
            {
                return Err(ContainerError::Other(anyhow!(
                    "Session does not belong to workspace"
                )));
            }

            let workspace = Workspace::find_by_id(&self.db().pool, session.workspace_id)
                .await?
                .ok_or(SqlxError::RowNotFound)?;

            let container_ref = match workspace.container_ref.as_deref() {
                Some(container_ref) if !container_ref.is_empty() => container_ref,
                _ => &self.ensure_container_exists(&workspace).await?,
            };

            if container_ref.is_empty() {
                return Err(ContainerError::Other(anyhow!("Workspace path is empty")));
            }

            let workspace_path = PathBuf::from(container_ref);
            let workdir = match session.agent_working_dir.as_deref() {
                Some(dir) if !dir.is_empty() => Some(workspace_path.join(dir)),
                _ => Some(workspace_path),
            };

            let repos =
                WorkspaceRepo::find_repos_for_workspace(&self.db().pool, session.workspace_id)
                    .await
                    .unwrap_or_default();
            let repo_path = if repos.len() == 1 {
                Some(repos[0].path.clone())
            } else {
                None
            };

            (workdir, repo_path)
        } else if workspace_id.is_some() {
            return Err(ContainerError::Other(anyhow!(
                "session_id is required when workspace_id is provided"
            )));
        } else if let Some(repo_id) = repo_id {
            let repo = Repo::find_by_id(&self.db().pool, repo_id)
                .await
                .ok()
                .flatten()
                .map(|repo| repo.path);
            (None, repo)
        } else {
            (None, None)
        };

        #[cfg(feature = "qa-mode")]
        {
            let _ = executor_profile_id;
            let _ = workdir;
            let _ = repo_path;
            return Ok(None);
        }
        #[cfg(not(feature = "qa-mode"))]
        {
            let executor =
                ExecutorConfigs::get_cached().get_coding_agent_or_default(&executor_profile_id);

            // Spawn background task to refresh global cache for this executor
            let base_agent = executors::executors::BaseCodingAgent::from(&executor);
            executors::executors::utils::spawn_global_cache_refresh_for_agent(base_agent);

            let stream = executor
                .discover_options(workdir.as_deref(), repo_path.as_deref())
                .await?;
            Ok(Some(stream))
        }
    }

    async fn store_db_stream_handle(&self, id: Uuid, handle: JoinHandle<()>);

    async fn take_db_stream_handle(&self, id: &Uuid) -> Option<JoinHandle<()>>;

    async fn create(&self, workspace: &Workspace) -> Result<ContainerRef, ContainerError>;

    async fn kill_all_running_processes(&self) -> Result<(), ContainerError>;

    /// Try to re-attach to a still-running process left behind by a previous
    /// server instance instead of interrupting it. Returns true when adopted.
    async fn try_adopt_execution(&self, _process: &ExecutionProcess) -> bool {
        false
    }

    async fn delete(&self, workspace: &Workspace) -> Result<(), ContainerError>;

    /// A context is finalized when
    /// - Always when the execution process has failed or been killed
    /// - Never when the run reason is persistent (DevServer, BackgroundHelper)
    /// - Never when a setup script has no next_action (parallel mode)
    /// - The next action is None (no follow-up actions)
    fn should_finalize(&self, ctx: &ExecutionContext) -> bool {
        // Never finalize persistent processes
        if ctx.execution_process.run_reason.is_persistent() {
            return false;
        }

        // Never finalize setup scripts without a next_action (parallel mode).
        // In sequential mode, setup scripts have next_action pointing to coding agent,
        // so they won't finalize anyway (handled by next_action.is_none() check below).
        let action = ctx.execution_process.executor_action().unwrap();
        if matches!(
            ctx.execution_process.run_reason,
            ExecutionProcessRunReason::SetupScript
        ) && action.next_action.is_none()
        {
            return false;
        }

        // Always finalize failed, killed or interrupted executions, regardless of next action
        if matches!(
            ctx.execution_process.status,
            ExecutionProcessStatus::Failed
                | ExecutionProcessStatus::Killed
                | ExecutionProcessStatus::Interrupted
                | ExecutionProcessStatus::Indeterminate
        ) {
            return true;
        }

        // Otherwise, finalize only if no next action
        action.next_action.is_none()
    }

    /// Finalize workspace execution by sending notifications
    async fn finalize_task(&self, ctx: &ExecutionContext) {
        // Skip notification if process was intentionally killed by user
        // or interrupted by a server shutdown/restart
        if matches!(
            ctx.execution_process.status,
            ExecutionProcessStatus::Killed
                | ExecutionProcessStatus::Interrupted
                | ExecutionProcessStatus::Indeterminate
        ) {
            return;
        }

        let workspace_name = ctx
            .workspace
            .name
            .as_deref()
            .unwrap_or(&ctx.workspace.branch);
        let title = format!("Workspace Complete: {}", workspace_name);
        let message = match ctx.execution_process.status {
            ExecutionProcessStatus::Completed => format!(
                "✅ '{}' completed successfully\nBranch: {:?}\nExecutor: {:?}",
                workspace_name, ctx.workspace.branch, ctx.session.executor
            ),
            ExecutionProcessStatus::Failed => format!(
                "❌ '{}' execution failed\nBranch: {:?}\nExecutor: {:?}",
                workspace_name, ctx.workspace.branch, ctx.session.executor
            ),
            _ => {
                tracing::warn!(
                    "Tried to notify workspace completion for {} but process is still running!",
                    ctx.workspace.id
                );
                return;
            }
        };
        self.notification_service()
            .notify(&title, &message, Some(ctx.workspace.id))
            .await;
    }

    /// Cleanup executions marked as running in the db, call at startup.
    /// Returns the processes that were marked as interrupted.
    async fn cleanup_orphan_executions(&self) -> Result<Vec<ExecutionProcess>, ContainerError> {
        let running_processes = ExecutionProcess::find_running(&self.db().pool).await?;
        let mut interrupted = Vec::new();
        for process in running_processes {
            // A running row owned by a worker is not a coordinator-local
            // orphan. Reconciliation is authoritative for that row; if it
            // remained running, evidence was unavailable and cleanup must
            // preserve both the process and workspace state.
            if ExecutionWorkerJob::find_by_execution_id(&self.db().pool, process.id)
                .await?
                .is_some()
            {
                tracing::info!(
                    execution_id = %process.id,
                    "Retaining worker-owned running execution during orphan cleanup"
                );
                continue;
            }
            // Detached processes (dev servers) are left running across
            // restarts; if still alive, re-attach instead of killing.
            if self.try_adopt_execution(&process).await {
                continue;
            }
            tracing::info!(
                "Found orphaned execution process {} for session {}",
                process.id,
                process.session_id
            );
            // If the previous server died uncleanly (crash/SIGKILL), the OS
            // process group may still be alive: kill it before touching state
            // so a restarted dev server won't fight it for ports.
            #[cfg(unix)]
            if let Some(pgid) = process.pgid {
                let age_secs = (chrono::Utc::now() - process.started_at).num_seconds();
                if utils::process::kill_orphan_process_group(pgid as i32, age_secs).await {
                    tracing::info!(
                        "Killed orphaned OS process group {} for execution process {}",
                        pgid,
                        process.id
                    );
                }
            }
            // Snapshot before recording HEAD or offering the process for
            // auto-resume. If capture fails after the orphan was killed, mark
            // the dead row interrupted so it cannot remain a phantom running
            // process, but do not record an after-state or return it as a
            // successfully recovered process.
            if let Err(e) = self.commit_interrupted_wip(&process).await {
                tracing::error!(
                    "Failed to preserve work for orphaned execution process {}: {}",
                    process.id,
                    e
                );
                if let Err(update_error) = ExecutionProcess::update_completion(
                    &self.db().pool,
                    process.id,
                    ExecutionProcessStatus::Interrupted,
                    None,
                )
                .await
                {
                    tracing::error!(
                        "Failed to mark unpreserved orphaned execution process {} interrupted: {}",
                        process.id,
                        update_error
                    );
                }
                continue;
            }

            if let Err(e) = ExecutionProcess::update_completion(
                &self.db().pool,
                process.id,
                ExecutionProcessStatus::Interrupted,
                None, // No exit code for orphaned processes
            )
            .await
            {
                tracing::error!(
                    "Failed to update orphaned execution process {} status: {}",
                    process.id,
                    e
                );
                continue;
            }
            // Capture after-head commit OID per repository
            if let Ok(ctx) = ExecutionProcess::load_context(&self.db().pool, process.id).await
                && let Some(ref container_ref) = ctx.workspace.container_ref
            {
                let workspace_root = PathBuf::from(container_ref);
                for repo in &ctx.repos {
                    let repo_path = workspace_root.join(&repo.name);
                    if let Ok(head) = self.git().get_head_info(&repo_path)
                        && let Err(err) = ExecutionProcessRepoState::update_after_head_commit(
                            &self.db().pool,
                            process.id,
                            repo.id,
                            &head.oid,
                        )
                        .await
                    {
                        tracing::warn!(
                            "Failed to update after_head_commit for repo {} on process {}: {}",
                            repo.id,
                            process.id,
                            err
                        );
                    }
                }
            }
            tracing::info!(
                "Marked orphaned execution process {} as interrupted",
                process.id
            );
            interrupted.push(process);
        }
        Ok(interrupted)
    }

    /// Restart dev servers that were interrupted by a server shutdown or
    /// crash. Call at startup with the processes returned by
    /// [`Self::cleanup_orphan_executions`].
    async fn restart_interrupted_dev_servers(&self, interrupted: &[ExecutionProcess]) {
        for process in interrupted {
            if process.run_reason != ExecutionProcessRunReason::DevServer {
                continue;
            }
            let ctx = match ExecutionProcess::load_context(&self.db().pool, process.id).await {
                Ok(ctx) => ctx,
                Err(e) => {
                    tracing::warn!(
                        "Skipping dev server restart for process {}: failed to load context: {}",
                        process.id,
                        e
                    );
                    continue;
                }
            };
            if ctx.workspace.archived || ctx.workspace.worktree_deleted {
                continue;
            }
            // Only restart into an existing worktree; don't recreate one at boot
            if !ctx
                .workspace
                .container_ref
                .as_deref()
                .is_some_and(|p| Path::new(p).exists())
            {
                continue;
            }
            let Ok(executor_action) = process.executor_action() else {
                continue;
            };
            match self
                .start_execution(
                    &ctx.workspace,
                    &ctx.session,
                    executor_action,
                    &ExecutionProcessRunReason::DevServer,
                )
                .await
            {
                Ok(new_process) => {
                    tracing::info!(
                        "Restarted interrupted dev server for workspace {} as process {}",
                        ctx.workspace.id,
                        new_process.id
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to restart interrupted dev server for workspace {}: {}",
                        ctx.workspace.id,
                        e
                    );
                }
            }
        }
    }

    /// Resume coding-agent runs that were interrupted by a server shutdown
    /// or crash by sending them the resume follow-up. Call at startup (only
    /// when opted in via config) with the processes returned by
    /// [`Self::cleanup_orphan_executions`]. Each run is resumed at most
    /// once: interrupted resume follow-ups are not resumed again, so a
    /// crash-restart loop cannot keep respawning agents.
    async fn resume_interrupted_coding_agents(&self, interrupted: &[ExecutionProcess]) {
        let mut resumed_sessions = HashSet::new();
        for process in interrupted {
            if process.run_reason != ExecutionProcessRunReason::CodingAgent {
                continue;
            }
            // One follow-up per session, even if several of its processes
            // were somehow marked interrupted.
            if !resumed_sessions.insert(process.session_id) {
                continue;
            }
            match self.resume_interrupted_coding_agent(process).await {
                Ok(Some(new_process)) => {
                    tracing::info!(
                        "Auto-resumed interrupted coding agent process {} as process {}",
                        process.id,
                        new_process.id
                    );
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(
                        "Failed to auto-resume interrupted coding agent process {}: {}",
                        process.id,
                        e
                    );
                }
            }
        }
    }

    /// Send the resume follow-up for a single interrupted coding-agent
    /// process. Returns `Ok(None)` when the process is intentionally
    /// skipped (already resumed once, workspace archived/deleted, or no
    /// agent session to resume).
    async fn resume_interrupted_coding_agent(
        &self,
        process: &ExecutionProcess,
    ) -> Result<Option<ExecutionProcess>, ContainerError> {
        let pool = &self.db().pool;
        let executor_action = process.executor_action().map_err(ContainerError::Other)?;
        let Some(executor_config) = executor_config_for_auto_resume(executor_action) else {
            tracing::info!(
                "Not auto-resuming process {}: not a coding-agent request, or it was itself a resume of an interrupted run",
                process.id
            );
            return Ok(None);
        };
        let ctx = ExecutionProcess::load_context(pool, process.id).await?;
        if ctx.workspace.archived || ctx.workspace.worktree_deleted {
            return Ok(None);
        }
        let Some(resume_info) =
            CodingAgentTurn::find_latest_session_info(pool, process.session_id).await?
        else {
            tracing::info!(
                "Not auto-resuming process {}: no agent session to resume",
                process.id
            );
            return Ok(None);
        };
        self.ensure_container_exists(&ctx.workspace).await?;
        let repos = WorkspaceRepo::find_repos_for_workspace(pool, ctx.workspace.id).await?;
        let cleanup_action = self.cleanup_actions_for_repos(&repos);
        let working_dir = ctx
            .session
            .agent_working_dir
            .as_ref()
            .filter(|dir| !dir.is_empty())
            .cloned();
        let action = ExecutorAction::new(
            ExecutorActionType::CodingAgentFollowUpRequest(CodingAgentFollowUpRequest {
                prompt: RESUME_INTERRUPTED_PROMPT.to_string(),
                session_id: resume_info.session_id,
                reset_to_message_id: None,
                executor_config,
                working_dir,
            }),
            cleanup_action.map(Box::new),
        );
        let new_process = self
            .start_execution(
                &ctx.workspace,
                &ctx.session,
                &action,
                &ExecutionProcessRunReason::CodingAgent,
            )
            .await?;
        Ok(Some(new_process))
    }

    /// Backfill before_head_commit for legacy execution processes.
    /// Rules:
    /// - If a process has after_head_commit and missing before_head_commit,
    ///   then set before_head_commit to the previous process's after_head_commit.
    /// - If there is no previous process, set before_head_commit to the base branch commit.
    async fn backfill_before_head_commits(&self) -> Result<(), ContainerError> {
        let pool = &self.db().pool;
        let rows = ExecutionProcess::list_missing_before_context(pool).await?;
        for row in rows {
            // Skip if no after commit at all (shouldn't happen due to WHERE)
            // Prefer previous process after-commit if present
            let mut before = row.prev_after_head_commit.clone();

            // Fallback to base branch commit OID
            if before.is_none() {
                let repo_path = std::path::Path::new(row.repo_path.as_deref().unwrap_or_default());
                match self
                    .git()
                    .get_branch_oid(repo_path, row.target_branch.as_str())
                {
                    Ok(oid) => before = Some(oid),
                    Err(e) => {
                        tracing::warn!(
                            "Backfill: Failed to resolve base branch OID for workspace {} (branch {}): {}",
                            row.workspace_id,
                            row.target_branch,
                            e
                        );
                    }
                }
            }

            if let Some(before_oid) = before
                && let Err(e) = ExecutionProcessRepoState::update_before_head_commit(
                    pool,
                    row.id,
                    row.repo_id,
                    &before_oid,
                )
                .await
            {
                tracing::warn!(
                    "Backfill: Failed to update before_head_commit for process {}: {}",
                    row.id,
                    e
                );
            }
        }

        Ok(())
    }

    /// Backfill repo names that were migrated with a sentinel placeholder.
    /// Also backfills dev_script_working_dir and agent_working_dir for single-repo projects.
    async fn backfill_repo_names(&self) -> Result<(), ContainerError> {
        let pool = &self.db().pool;
        let repos = Repo::list_needing_name_fix(pool).await?;

        if repos.is_empty() {
            return Ok(());
        }

        tracing::info!("Backfilling {} repo names", repos.len());

        for repo in repos {
            let name = repo
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&repo.id.to_string())
                .to_string();

            Repo::update_name(pool, repo.id, &name, &name).await?;
        }

        Ok(())
    }

    fn cleanup_actions_for_repos(&self, repos: &[Repo]) -> Option<ExecutorAction> {
        let repos_with_cleanup: Vec<_> = repos
            .iter()
            .filter(|r| r.cleanup_script.is_some())
            .collect();

        if repos_with_cleanup.is_empty() {
            return None;
        }

        let mut iter = repos_with_cleanup.iter();
        let first = iter.next()?;
        let mut root_action = ExecutorAction::new(
            ExecutorActionType::ScriptRequest(ScriptRequest {
                script: first.cleanup_script.clone().unwrap(),
                language: ScriptRequestLanguage::Bash,
                context: ScriptContext::CleanupScript,
                working_dir: Some(first.name.clone()),
            }),
            None,
        );

        for repo in iter {
            root_action = root_action.append_action(ExecutorAction::new(
                ExecutorActionType::ScriptRequest(ScriptRequest {
                    script: repo.cleanup_script.clone().unwrap(),
                    language: ScriptRequestLanguage::Bash,
                    context: ScriptContext::CleanupScript,
                    working_dir: Some(repo.name.clone()),
                }),
                None,
            ));
        }

        Some(root_action)
    }

    fn archive_actions_for_repos(&self, repos: &[Repo]) -> Option<ExecutorAction> {
        let repos_with_archive: Vec<_> = repos
            .iter()
            .filter(|r| r.archive_script.is_some())
            .collect();

        if repos_with_archive.is_empty() {
            return None;
        }

        let mut iter = repos_with_archive.iter();
        let first = iter.next()?;
        let mut root_action = ExecutorAction::new(
            ExecutorActionType::ScriptRequest(ScriptRequest {
                script: first.archive_script.clone().unwrap(),
                language: ScriptRequestLanguage::Bash,
                context: ScriptContext::ArchiveScript,
                working_dir: Some(first.name.clone()),
            }),
            None,
        );

        for repo in iter {
            root_action = root_action.append_action(ExecutorAction::new(
                ExecutorActionType::ScriptRequest(ScriptRequest {
                    script: repo.archive_script.clone().unwrap(),
                    language: ScriptRequestLanguage::Bash,
                    context: ScriptContext::ArchiveScript,
                    working_dir: Some(repo.name.clone()),
                }),
                None,
            ));
        }

        Some(root_action)
    }

    /// Attempts to run the archive script for a workspace if configured.
    /// Silently returns Ok if no archive script is configured or if conditions aren't met.
    async fn try_run_archive_script(&self, workspace_id: Uuid) -> Result<(), ContainerError> {
        let pool = &self.db().pool;
        let workspace = Workspace::find_by_id(pool, workspace_id)
            .await?
            .ok_or(ContainerError::Other(anyhow!("Workspace not found")))?;
        if ExecutionProcess::has_running_non_persistent_processes_for_workspace(pool, workspace.id)
            .await
            .unwrap_or(true)
        {
            return Ok(());
        }
        if self.ensure_container_exists(&workspace).await.is_err() {
            return Ok(());
        }
        let repos = WorkspaceRepo::find_repos_for_workspace(pool, workspace.id).await?;
        let Some(action) = self.archive_actions_for_repos(&repos) else {
            return Ok(());
        };
        let session = match Session::find_latest_by_workspace_id(pool, workspace.id).await? {
            Some(s) => s,
            None => {
                Session::create(
                    pool,
                    &CreateSession {
                        executor: None,
                        name: None,
                    },
                    Uuid::new_v4(),
                    workspace.id,
                )
                .await?
            }
        };
        self.start_execution(
            &workspace,
            &session,
            &action,
            &ExecutionProcessRunReason::ArchiveScript,
        )
        .await?;

        Ok(())
    }

    /// Archive a workspace: set archived flag, stop running persistent
    /// processes (dev servers, background helpers), and run archive script.
    async fn archive_workspace(&self, workspace_id: Uuid) -> Result<(), ContainerError> {
        let pool = &self.db().pool;

        Workspace::set_archived(pool, workspace_id, true).await?;

        // Stop running dev servers and background helpers
        for run_reason in [
            ExecutionProcessRunReason::DevServer,
            ExecutionProcessRunReason::BackgroundHelper,
        ] {
            let Ok(processes) = ExecutionProcess::find_running_by_workspace_and_run_reason(
                pool,
                workspace_id,
                &run_reason,
            )
            .await
            else {
                continue;
            };
            for process in processes {
                if let Err(e) = self
                    .stop_execution(&process, ExecutionProcessStatus::Killed)
                    .await
                {
                    tracing::error!(
                        "Failed to stop {:?} {} for workspace {}: {}",
                        run_reason,
                        process.id,
                        workspace_id,
                        e
                    );
                }
            }
        }

        // Run archive script (silently skips if not configured)
        if let Err(e) = self.try_run_archive_script(workspace_id).await {
            tracing::error!(
                "Failed to run archive script for workspace {}: {}",
                workspace_id,
                e
            );
        }

        Ok(())
    }

    fn setup_actions_for_repos(&self, repos: &[Repo]) -> Option<ExecutorAction> {
        let repos_with_setup: Vec<_> = repos.iter().filter(|r| r.setup_script.is_some()).collect();

        if repos_with_setup.is_empty() {
            return None;
        }

        let mut iter = repos_with_setup.iter();
        let first = iter.next()?;
        let mut root_action = ExecutorAction::new(
            ExecutorActionType::ScriptRequest(ScriptRequest {
                script: first.setup_script.clone().unwrap(),
                language: ScriptRequestLanguage::Bash,
                context: ScriptContext::SetupScript,
                working_dir: Some(first.name.clone()),
            }),
            None,
        );

        for repo in iter {
            root_action = root_action.append_action(ExecutorAction::new(
                ExecutorActionType::ScriptRequest(ScriptRequest {
                    script: repo.setup_script.clone().unwrap(),
                    language: ScriptRequestLanguage::Bash,
                    context: ScriptContext::SetupScript,
                    working_dir: Some(repo.name.clone()),
                }),
                None,
            ));
        }

        Some(root_action)
    }

    fn setup_action_for_repo(repo: &Repo) -> Option<ExecutorAction> {
        repo.setup_script.as_ref().map(|script| {
            ExecutorAction::new(
                ExecutorActionType::ScriptRequest(ScriptRequest {
                    script: script.clone(),
                    language: ScriptRequestLanguage::Bash,
                    context: ScriptContext::SetupScript,
                    working_dir: Some(repo.name.clone()),
                }),
                None,
            )
        })
    }

    fn build_sequential_setup_chain(
        repos: &[&Repo],
        next_action: ExecutorAction,
    ) -> ExecutorAction {
        let mut chained = next_action;
        for repo in repos.iter().rev() {
            if let Some(script) = &repo.setup_script {
                chained = ExecutorAction::new(
                    ExecutorActionType::ScriptRequest(ScriptRequest {
                        script: script.clone(),
                        language: ScriptRequestLanguage::Bash,
                        context: ScriptContext::SetupScript,
                        working_dir: Some(repo.name.clone()),
                    }),
                    Some(Box::new(chained)),
                );
            }
        }
        chained
    }

    /// Reset a session to a specific process: restore worktrees, stop processes, drop later processes.
    async fn reset_session_to_process(
        &self,
        session_id: Uuid,
        target_process_id: Uuid,
        perform_git_reset: bool,
        force_when_dirty: bool,
    ) -> Result<(), ContainerError> {
        let pool = &self.db().pool;

        let process = ExecutionProcess::find_by_id(pool, target_process_id)
            .await?
            .ok_or_else(|| ContainerError::Other(anyhow!("Process not found")))?;
        if process.session_id != session_id {
            return Err(ContainerError::Other(anyhow!(
                "Process does not belong to this session"
            )));
        }

        let session = Session::find_by_id(pool, session_id)
            .await?
            .ok_or_else(|| ContainerError::Other(anyhow!("Session not found")))?;
        let workspace = Workspace::find_by_id(pool, session.workspace_id)
            .await?
            .ok_or_else(|| ContainerError::Other(anyhow!("Workspace not found")))?;

        let repos = WorkspaceRepo::find_repos_for_workspace(pool, workspace.id).await?;
        let repo_states =
            ExecutionProcessRepoState::find_by_execution_process_id(pool, target_process_id)
                .await?;

        let container_ref = self.ensure_container_exists(&workspace).await?;
        let workspace_dir = std::path::PathBuf::from(container_ref);
        let is_dirty = self
            .is_container_clean(&workspace)
            .await
            .map(|is_clean| !is_clean)
            .unwrap_or(false);

        if reset_would_discard_uncommitted_work(perform_git_reset, is_dirty, force_when_dirty) {
            return Err(ContainerError::Other(anyhow!(
                "Worktree has uncommitted changes; reset was refused to avoid data loss. Preserve the changes or retry with force_when_dirty=true to discard them."
            )));
        }

        for repo in &repos {
            let repo_state = repo_states.iter().find(|s| s.repo_id == repo.id);
            let target_oid = match repo_state.and_then(|s| s.before_head_commit.clone()) {
                Some(oid) => Some(oid),
                None => {
                    ExecutionProcess::find_prev_after_head_commit(
                        pool,
                        session_id,
                        target_process_id,
                        repo.id,
                    )
                    .await?
                }
            };

            let worktree_path = workspace_dir.join(&repo.name);
            if let Some(oid) = target_oid {
                self.git().reconcile_worktree_to_commit(
                    &worktree_path,
                    &oid,
                    git::WorktreeResetOptions::new(
                        perform_git_reset,
                        force_when_dirty,
                        is_dirty,
                        perform_git_reset,
                    ),
                );
            }
        }

        self.try_stop(&workspace, false).await;
        ExecutionProcess::drop_at_and_after(pool, session_id, target_process_id).await?;

        Ok(())
    }

    async fn try_stop(&self, workspace: &Workspace, include_dev_server: bool) {
        // stop execution processes for this workspace's sessions
        let sessions = match Session::find_by_workspace_id(&self.db().pool, workspace.id).await {
            Ok(s) => s,
            Err(_) => return,
        };

        for session in sessions {
            // Reap any warm (kept-alive) app-server parked for this session. Its
            // turn row is `Completed`, so the `Running`-only loop below would skip
            // it and the process would outlive the torn-down workspace — the leak
            // this hook closes (spec FR-4b, task 826e). Default no-op; the local
            // container overrides it to drain its warm registry.
            self.reap_warm_processes_for_session(session.id).await;

            if let Ok(processes) =
                ExecutionProcess::find_by_session_id(&self.db().pool, session.id, false).await
            {
                for process in processes {
                    // Skip persistent processes (dev servers, background
                    // helpers) unless explicitly included
                    if !include_dev_server && process.run_reason.is_persistent() {
                        continue;
                    }
                    if process.status == ExecutionProcessStatus::Running {
                        self.stop_execution(&process, ExecutionProcessStatus::Killed)
                            .await
                            .unwrap_or_else(|e| {
                                tracing::debug!(
                                    "Failed to stop execution process {} for workspace {}: {}",
                                    process.id,
                                    workspace.id,
                                    e
                                );
                            });
                    }
                }
            }
        }
    }

    /// Reap any warm (kept-alive) app-server parked for this session (Phase 2).
    /// Default no-op so non-local `ContainerService` impls are unaffected;
    /// `LocalContainerService` overrides it to drain its warm registry. Called
    /// from `try_stop` because a warm process's turn row is `Completed` and the
    /// `Running`-only stop loop would otherwise leak it. See
    /// `specs/vk/826e-coding-agent-war/`.
    async fn reap_warm_processes_for_session(&self, _session_id: Uuid) {}

    async fn ensure_container_exists(
        &self,
        workspace: &Workspace,
    ) -> Result<ContainerRef, ContainerError>;

    async fn is_container_clean(&self, workspace: &Workspace) -> Result<bool, ContainerError>;

    async fn start_execution_inner(
        &self,
        workspace: &Workspace,
        execution_process: &ExecutionProcess,
        executor_action: &ExecutorAction,
    ) -> Result<(), ContainerError>;

    /// Selects the process owner for an execution. Local implementations keep
    /// their existing spawn path by default; clustered implementations may
    /// override this to dispatch to the workspace's persisted worker.
    async fn dispatch_execution(
        &self,
        workspace: &Workspace,
        execution_process: &ExecutionProcess,
        executor_action: &ExecutorAction,
    ) -> Result<(), ContainerError> {
        self.start_execution_inner(workspace, execution_process, executor_action)
            .await
    }

    async fn stop_execution(
        &self,
        execution_process: &ExecutionProcess,
        status: ExecutionProcessStatus,
    ) -> Result<(), ContainerError>;

    async fn try_commit_changes(&self, ctx: &ExecutionContext) -> Result<bool, ContainerError>;

    /// Snapshot uncommitted worktree changes left behind by an interrupted run
    /// into a commit on the workspace branch, so they survive a restart. This
    /// mirrors the auto-commit that happens after a successful run and is a
    /// no-op for run reasons that don't produce worktree changes.
    async fn commit_interrupted_wip(
        &self,
        process: &ExecutionProcess,
    ) -> Result<(), ContainerError>;

    async fn copy_project_files(
        &self,
        source_dir: &Path,
        target_dir: &Path,
        copy_files: &str,
    ) -> Result<(), ContainerError>;

    /// Stream diff updates as LogMsg for WebSocket endpoints.
    async fn stream_diff(
        &self,
        workspace: &Workspace,
        stats_only: bool,
    ) -> Result<futures::stream::BoxStream<'static, Result<LogMsg, std::io::Error>>, ContainerError>;

    /// Fetch the MsgStore for a given execution ID, panicking if missing.
    async fn get_msg_store_by_id(&self, uuid: &Uuid) -> Option<Arc<MsgStore>> {
        let map = self.msg_stores().read().await;
        map.get(uuid).cloned()
    }

    async fn git_branch_prefix(&self) -> String;

    async fn git_branch_from_workspace(&self, workspace_id: &Uuid, task_title: &str) -> String {
        let task_title_id = git_branch_id(task_title);
        let prefix = self.git_branch_prefix().await;

        if prefix.is_empty() {
            format!("{}-{}", short_uuid(workspace_id), task_title_id)
        } else {
            format!("{}/{}-{}", prefix, short_uuid(workspace_id), task_title_id)
        }
    }

    async fn stream_raw_logs(
        &self,
        id: &Uuid,
    ) -> Option<futures::stream::BoxStream<'static, Result<LogMsg, std::io::Error>>> {
        if let Some(store) = self.get_msg_store_by_id(id).await {
            // First try in-memory store
            return Some(
                store
                    .history_plus_stream()
                    .filter(|msg| {
                        future::ready(matches!(
                            msg,
                            Ok(LogMsg::Stdout(..) | LogMsg::Stderr(..) | LogMsg::Finished) | Err(_)
                        ))
                    })
                    .boxed(),
            );
        } else {
            let messages = execution_process::load_raw_log_messages(&self.db().pool, *id).await?;

            let stream = futures::stream::iter(
                messages
                    .into_iter()
                    .filter(|m| matches!(m, LogMsg::Stdout(_) | LogMsg::Stderr(_)))
                    .chain(std::iter::once(LogMsg::Finished))
                    .map(Ok::<_, std::io::Error>),
            )
            .boxed();

            Some(stream)
        }
    }

    async fn stream_normalized_logs(
        &self,
        id: &Uuid,
    ) -> Option<futures::stream::BoxStream<'static, Result<LogMsg, std::io::Error>>> {
        // First try in-memory store (existing behavior)
        if let Some(store) = self.get_msg_store_by_id(id).await {
            Some(
                store
                    .history_plus_stream() // BoxStream<Result<LogMsg, io::Error>>
                    .filter(|msg| future::ready(matches!(msg, Ok(LogMsg::JsonPatch(..)) | Err(_))))
                    .chain(futures::stream::once(async {
                        Ok::<_, std::io::Error>(LogMsg::Finished)
                    }))
                    .boxed(),
            )
        } else {
            let process = match ExecutionProcess::find_by_id(&self.db().pool, *id).await {
                Ok(Some(process)) => process,
                Ok(None) => {
                    tracing::error!("No execution process found for ID: {}", id);
                    return None;
                }
                Err(e) => {
                    tracing::error!("Failed to fetch execution process {}: {}", id, e);
                    return None;
                }
            };

            // A finished process that has already been materialized is served
            // from its settled entries. Deliberately before the permit is taken:
            // a cache hit does no normalization, so making it queue behind the
            // runs that do would reintroduce the wait this exists to remove. It
            // also skips `ensure_container_exists` below — recreating a worktree
            // to read a conversation is pure cost once the answer is stored.
            let cache_path =
                utils::execution_logs::process_normalized_log_file_path(process.session_id, *id);
            if let Some(stream) = replay_materialized_log(&cache_path, *id, "optimistic").await {
                return Some(stream);
            }

            tracing::info!(
                execution_id = %id,
                "Normalized log cache missed; waiting for execution materialization ownership"
            );
            let lease = HISTORICAL_NORMALIZATIONS.acquire(*id).await;
            let ownership_wait = lease.wait_started.elapsed();
            tracing::info!(
                execution_id = %id,
                joined_existing = lease.joined_existing,
                wait_ms = ownership_wait.as_millis(),
                "Acquired historical log materialization ownership"
            );

            // Another reader may have completed and atomically published the
            // sidecar while this reader waited for execution ownership.
            if let Some(stream) = replay_materialized_log(&cache_path, *id, "after_wait").await {
                return Some(stream);
            }

            let capacity_wait_started = Instant::now();
            let permit = HISTORICAL_NORMALIZATION_PERMITS
                .clone()
                .acquire_owned()
                .await
                .ok()?;
            tracing::info!(
                execution_id = %id,
                wait_ms = capacity_wait_started.elapsed().as_millis(),
                "Acquired historical log normalization capacity"
            );
            let raw_messages =
                execution_process::load_raw_log_messages(&self.db().pool, *id).await?;
            let total_messages = raw_messages.len();
            // Bound the history before anything is materialized. Without this a
            // single long run can exhaust the server's memory every time a
            // client reconnects to its log stream.
            let (messages, dropped) = utils::execution_logs::cap_normalizable_history(
                raw_messages,
                utils::execution_logs::MAX_HISTORICAL_NORMALIZATION_MSGS,
            );
            tracing::info!(
                execution_id = %id,
                message_count = messages.len(),
                total_messages,
                dropped_messages = dropped,
                "Starting bounded historical log normalization"
            );

            // Create temporary store and populate. Messages are pre-filtered to
            // the normalizable variants (Stdout/Stderr, plus JsonPatch which is
            // already normalized) and capped to the newest window.
            let temp_store = Arc::new(MsgStore::new());
            if dropped > 0 {
                tracing::warn!(
                    execution_id = %id,
                    dropped_messages = dropped,
                    total_messages,
                    "Historical log too large to normalize in full; showing the most recent messages"
                );
                // Tell the reader their view is partial rather than silently
                // starting mid-conversation.
                temp_store.push(LogMsg::Stdout(format!(
                    "[vibe-kanban] {dropped} earlier log messages omitted \
                     (showing the most recent {} of {total_messages}).\n",
                    messages.len()
                )));
            }
            for msg in messages {
                temp_store.push(msg);
            }
            temp_store.push_finished();

            // Get the workspace to determine correct directory
            let (workspace, _session) =
                match process.parent_workspace_and_session(&self.db().pool).await {
                    Ok(Some((workspace, session))) => (workspace, session),
                    Ok(None) => {
                        tracing::error!(
                            "No workspace/session found for session ID: {}",
                            process.session_id
                        );
                        return None;
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to fetch workspace for session {}: {}",
                            process.session_id,
                            e
                        );
                        return None;
                    }
                };

            if let Err(err) = self.ensure_container_exists(&workspace).await {
                tracing::warn!(
                    "Failed to recreate worktree before log normalization for workspace {}: {}",
                    workspace.id,
                    err
                );
            }

            let current_dir = self.workspace_to_current_dir(&workspace);

            let executor_action = if let Ok(executor_action) = process.executor_action() {
                executor_action
            } else {
                tracing::error!(
                    "Failed to parse executor action: {:?}",
                    process.executor_action()
                );
                return None;
            };

            // Spawn normalizer on populated store and collect JoinHandles
            let handles = match executor_action.typ() {
                ExecutorActionType::CodingAgentInitialRequest(request) => {
                    #[cfg(feature = "qa-mode")]
                    {
                        let executor = QaMockExecutor;
                        executor.normalize_logs(
                            temp_store.clone(),
                            &request.effective_dir(&current_dir),
                        )
                    }
                    #[cfg(not(feature = "qa-mode"))]
                    {
                        let executor = ExecutorConfigs::get_cached()
                            .get_coding_agent_or_default(&request.executor_config.profile_id());
                        executor.normalize_logs(
                            temp_store.clone(),
                            &request.effective_dir(&current_dir),
                        )
                    }
                }
                ExecutorActionType::CodingAgentFollowUpRequest(request) => {
                    #[cfg(feature = "qa-mode")]
                    {
                        let executor = QaMockExecutor;
                        executor.normalize_logs(
                            temp_store.clone(),
                            &request.effective_dir(&current_dir),
                        )
                    }
                    #[cfg(not(feature = "qa-mode"))]
                    {
                        let executor = ExecutorConfigs::get_cached()
                            .get_coding_agent_or_default(&request.executor_config.profile_id());
                        executor.normalize_logs(
                            temp_store.clone(),
                            &request.effective_dir(&current_dir),
                        )
                    }
                }
                #[cfg(feature = "qa-mode")]
                ExecutorActionType::ReviewRequest(_request) => {
                    let executor = QaMockExecutor;
                    executor.normalize_logs(temp_store.clone(), &current_dir)
                }
                #[cfg(not(feature = "qa-mode"))]
                ExecutorActionType::ReviewRequest(request) => {
                    let executor = ExecutorConfigs::get_cached()
                        .get_coding_agent_or_default(&request.executor_config.profile_id());
                    executor.normalize_logs(temp_store.clone(), &current_dir)
                }
                _ => {
                    tracing::debug!(
                        "Executor action doesn't support log normalization: {:?}",
                        process.executor_action()
                    );
                    return None;
                }
            };

            // Await all normalizer tasks, then push Ready so the dedup
            // stream knows when to flush its buffer and terminate.
            let mut task_abort_handles: Vec<_> =
                handles.iter().map(JoinHandle::abort_handle).collect();
            {
                let store = temp_store.clone();
                let completion_task = tokio::spawn(async move {
                    for handle in handles {
                        let _ = handle.await;
                    }
                    store.push(LogMsg::Ready);
                });
                task_abort_handles.push(completion_task.abort_handle());
            }
            let completed = Arc::new(AtomicBool::new(false));
            let lifetime = HistoricalNormalizationLifetime {
                _permit: permit,
                _lease: lease,
                tasks: task_abort_handles,
                execution_id: *id,
                completed: completed.clone(),
            };

            // Stream normalized patches, deduplicating consecutive patches
            // that target the same path (only the final state matters for
            // historical replay). The Ready sentinel flushes the buffer.
            enum PatchOrDone {
                Patch(Patch),
                Done,
            }

            let stream = temp_store
                .history_plus_stream()
                .filter_map(|msg| async move {
                    match msg {
                        Ok(LogMsg::JsonPatch(patch)) => Some(PatchOrDone::Patch(patch)),
                        Ok(LogMsg::Ready) => Some(PatchOrDone::Done),
                        _ => None,
                    }
                });

            let deduped = futures::stream::unfold(
                (stream.boxed(), None::<Patch>, HashSet::<String>::new()),
                |(mut stream, buffered, mut sent_paths)| async move {
                    match stream.next().await {
                        Some(PatchOrDone::Patch(patch)) => {
                            let Some(prev) = buffered else {
                                // First patch — just buffer it
                                return Some((None, (stream, Some(patch), sent_paths)));
                            };
                            if patch_entry_path(&patch) == patch_entry_path(&prev)
                                && is_add_or_replace(&patch)
                                && is_add_or_replace(&prev)
                            {
                                // Same path, both add/replace — replace buffer
                                Some((None, (stream, Some(patch), sent_paths)))
                            } else {
                                // Different — emit prev, buffer new
                                let prev = fix_patch_ops(prev, &mut sent_paths);
                                Some((Some(prev), (stream, Some(patch), sent_paths)))
                            }
                        }
                        Some(PatchOrDone::Done) | None => {
                            // Sentinel or stream end: flush buffer and terminate
                            if let Some(prev) = buffered {
                                let prev = fix_patch_ops(prev, &mut sent_paths);
                                return Some((Some(prev), (stream, None, sent_paths)));
                            }
                            None
                        }
                    }
                },
            )
            .filter_map(|opt| async move { opt })
            .map(|p| Ok::<_, std::io::Error>(LogMsg::JsonPatch(p)))
            // The closure owns the permit and abort handles. Dropping the
            // WebSocket stream therefore cancels replay instead of leaving
            // detached normalizers consuming memory.
            .map(move |item| {
                let _keep_alive = &lifetime;
                item
            });

            // Materialize what this replay produced, so the next reader pays
            // none of it. Collected as the reader consumes the stream, and
            // written only when the stream reaches its end: a reader that
            // disconnects halfway has seen a partial conversation, and storing
            // that would turn a transient disconnect into a permanently short
            // transcript.
            let collected: Arc<std::sync::Mutex<Vec<Patch>>> = Arc::default();
            let collector = collected.clone();
            let was_truncated = dropped > 0;
            // Only a process that has actually stopped is safe to store. Being
            // on this branch does not prove it has: a process whose in-memory
            // store did not survive a server restart reaches here while still
            // running, and caching that would freeze a live conversation at
            // whatever had been written when the server came back.
            let is_finished = process.status != ExecutionProcessStatus::Running;
            let execution_id = *id;
            let deduped = deduped
                .map(move |item| {
                    if let Ok(LogMsg::JsonPatch(patch)) = &item
                        && let Ok(mut patches) = collector.lock()
                    {
                        patches.push(patch.clone());
                    }
                    item
                })
                .chain(futures::stream::once(async move {
                    if is_finished {
                        let patches = collected
                            .lock()
                            .map(|patches| patches.clone())
                            .unwrap_or_default();
                        let patch_count = patches.len();
                        let stored =
                            materialize_normalized_log(&cache_path, &patches, was_truncated).await;
                        tracing::info!(
                            execution_id = %execution_id,
                            patch_count,
                            stored,
                            truncated = was_truncated,
                            "Historical log normalization completed"
                        );
                    }
                    completed.store(true, Ordering::Relaxed);
                    Ok::<_, std::io::Error>(LogMsg::Finished)
                }));

            Some(deduped.boxed())
        }
    }

    /// Returns the settled normalized entries for an execution, for callers
    /// that want a `Vec` rather than a live stream (e.g. an MCP tool
    /// answering "what did this turn say").
    ///
    /// Reuses [`Self::stream_normalized_logs`] rather than a second read
    /// path: the same in-memory store / on-disk materialized cache / bounded
    /// historical re-normalization decision it makes applies here too. Only
    /// `/entries/<index>` patches are kept before materializing — repo-diff
    /// patches (`/entries/<repo>/<file>`) target a nested object, not the
    /// `{"entries": []}` array `materialize_entries` applies against, and
    /// they aren't messages anyway.
    async fn normalized_entries(&self, id: &Uuid) -> Option<Vec<NormalizedEntry>> {
        let mut stream = self.stream_normalized_logs(id).await?;
        let mut patches = Vec::new();
        while let Some(item) = stream.next().await {
            match item {
                Ok(LogMsg::JsonPatch(patch)) => {
                    if is_indexed_entry_patch(&patch) {
                        patches.push(patch);
                    }
                }
                Ok(LogMsg::Finished) => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }

        match normalized_log_cache::materialize_entries(&patches) {
            Ok(values) => Some(
                values
                    .into_iter()
                    .filter_map(|value| serde_json::from_value::<NormalizedEntry>(value).ok())
                    .collect(),
            ),
            Err(e) => {
                tracing::warn!(
                    execution_id = %id,
                    "Could not materialize normalized entries: {e}"
                );
                None
            }
        }
    }

    async fn start_workspace(
        &self,
        workspace: &Workspace,
        executor_config: ExecutorConfig,
        prompt: String,
    ) -> Result<ExecutionProcess, ContainerError> {
        // Create container
        self.create(workspace).await?;

        let repos = WorkspaceRepo::find_repos_for_workspace(&self.db().pool, workspace.id).await?;

        let workspace = Workspace::find_by_id(&self.db().pool, workspace.id)
            .await?
            .ok_or(SqlxError::RowNotFound)?;

        // Create a session for this workspace
        let session = Session::create(
            &self.db().pool,
            &CreateSession {
                executor: Some(executor_config.executor.to_string()),
                name: None,
            },
            Uuid::new_v4(),
            workspace.id,
        )
        .await?;

        let repos_with_setup: Vec<_> = repos.iter().filter(|r| r.setup_script.is_some()).collect();

        let all_parallel = repos_with_setup.iter().all(|r| r.parallel_setup_script);

        let cleanup_action = self.cleanup_actions_for_repos(&repos);

        let working_dir = session
            .agent_working_dir
            .as_ref()
            .filter(|dir| !dir.is_empty())
            .cloned();

        // Several projects can share a single repository (e.g. different
        // services in a homelab monorepo). When this workspace targets a
        // subdirectory of a shared repo, tell the agent which one it is on.
        let prompt = scope_initial_prompt_to_working_dir(prompt, &repos);

        let coding_action = ExecutorAction::new(
            ExecutorActionType::CodingAgentInitialRequest(CodingAgentInitialRequest {
                prompt,
                executor_config: executor_config.clone(),
                working_dir,
            }),
            cleanup_action.map(Box::new),
        );

        let execution_process = if all_parallel {
            // All parallel: start each setup independently, then start coding agent
            for repo in &repos_with_setup {
                if let Some(action) = Self::setup_action_for_repo(repo)
                    && let Err(e) = self
                        .start_execution(
                            &workspace,
                            &session,
                            &action,
                            &ExecutionProcessRunReason::SetupScript,
                        )
                        .await
                {
                    tracing::warn!(?e, "Failed to start setup script in parallel mode");
                }
            }
            self.start_execution(
                &workspace,
                &session,
                &coding_action,
                &ExecutionProcessRunReason::CodingAgent,
            )
            .await?
        } else {
            // Any sequential: chain ALL setups → coding agent via next_action
            let main_action = Self::build_sequential_setup_chain(&repos_with_setup, coding_action);
            self.start_execution(
                &workspace,
                &session,
                &main_action,
                &ExecutionProcessRunReason::SetupScript,
            )
            .await?
        };

        Ok(execution_process)
    }

    async fn start_execution(
        &self,
        workspace: &Workspace,
        session: &Session,
        executor_action: &ExecutorAction,
        run_reason: &ExecutionProcessRunReason,
    ) -> Result<ExecutionProcess, ContainerError> {
        self.start_execution_with_id(
            workspace,
            session,
            executor_action,
            run_reason,
            Uuid::new_v4(),
        )
        .await
    }

    /// Start an execution with a caller-owned durable identity. Lifecycle
    /// transitions that cross an HTTP retry boundary use this to make process
    /// creation idempotent instead of creating a second agent after a lost
    /// response.
    async fn start_execution_with_id(
        &self,
        workspace: &Workspace,
        session: &Session,
        executor_action: &ExecutorAction,
        run_reason: &ExecutionProcessRunReason,
        execution_process_id: Uuid,
    ) -> Result<ExecutionProcess, ContainerError> {
        // Create new execution process record
        // Capture current HEAD per repository as the "before" commit for this execution
        let repositories =
            WorkspaceRepo::find_repos_for_workspace(&self.db().pool, workspace.id).await?;
        if repositories.is_empty() {
            return Err(ContainerError::Other(anyhow!(
                "Workspace has no repositories configured"
            )));
        }

        let workspace_root = workspace
            .container_ref
            .as_ref()
            .map(std::path::PathBuf::from)
            .ok_or_else(|| ContainerError::Other(anyhow!("Container ref not found")))?;

        let mut repo_states = Vec::with_capacity(repositories.len());
        for repo in &repositories {
            let repo_path = workspace_root.join(&repo.name);
            let before_head_commit = self.git().get_head_info(&repo_path).ok().map(|h| h.oid);
            repo_states.push(CreateExecutionProcessRepoState {
                repo_id: repo.id,
                before_head_commit,
                after_head_commit: None,
                merge_commit: None,
            });
        }
        let create_execution_process = CreateExecutionProcess {
            session_id: session.id,
            executor_action: executor_action.clone(),
            run_reason: run_reason.clone(),
        };

        let execution_process = ExecutionProcess::create(
            &self.db().pool,
            &create_execution_process,
            execution_process_id,
            &repo_states,
        )
        .await?;
        if *run_reason != ExecutionProcessRunReason::ArchiveScript {
            Workspace::set_archived(&self.db().pool, workspace.id, false).await?;
        }

        if let Some(prompt) = match executor_action.typ() {
            ExecutorActionType::CodingAgentInitialRequest(coding_agent_request) => {
                Some(coding_agent_request.prompt.clone())
            }
            ExecutorActionType::CodingAgentFollowUpRequest(follow_up_request) => {
                Some(follow_up_request.prompt.clone())
            }
            ExecutorActionType::ReviewRequest(review_request) => {
                Some(review_request.prompt.clone())
            }
            ExecutorActionType::ScriptRequest(_) => None,
        } {
            let create_coding_agent_turn = CreateCodingAgentTurn {
                execution_process_id: execution_process.id,
                prompt: Some(prompt),
            };

            let coding_agent_turn_id = Uuid::new_v4();

            CodingAgentTurn::create(
                &self.db().pool,
                &create_coding_agent_turn,
                coding_agent_turn_id,
            )
            .await?;
        }

        if let Err(start_error) = self
            .dispatch_execution(workspace, &execution_process, executor_action)
            .await
        {
            // Mark process as failed
            if let Err(update_error) = ExecutionProcess::update_completion(
                &self.db().pool,
                execution_process.id,
                ExecutionProcessStatus::Failed,
                None,
            )
            .await
            {
                tracing::error!(
                    "Failed to mark execution process {} as failed after start error: {}",
                    execution_process.id,
                    update_error
                );
            }
            // Emit stderr error message
            let log_message = LogMsg::Stderr(format!("Failed to start execution: {start_error}"));
            if let Err(e) = execution_process::append_log_message(
                session.id,
                execution_process.id,
                &log_message,
            )
            .await
            {
                tracing::error!(
                    "Failed to write error log for execution {}: {}",
                    execution_process.id,
                    e
                );
            }

            // Emit NextAction with failure context for coding agent requests
            if let ContainerError::ExecutorError(ExecutorError::ExecutableNotFound { program }) =
                &start_error
            {
                let help_text = format!("The required executable `{program}` is not installed.");
                let error_message = NormalizedEntry {
                    timestamp: None,
                    entry_type: NormalizedEntryType::ErrorMessage {
                        error_type: NormalizedEntryError::SetupRequired,
                    },
                    content: help_text,
                    metadata: None,
                };
                let patch = ConversationPatch::add_normalized_entry(2, error_message);
                if let Err(e) = execution_process::append_log_message(
                    session.id,
                    execution_process.id,
                    &LogMsg::JsonPatch(patch),
                )
                .await
                {
                    tracing::error!(
                        "Failed to write setup-required log for execution {}: {}",
                        execution_process.id,
                        e
                    );
                }
            };
            return Err(start_error);
        }

        // Start processing normalised logs for executor requests and follow ups
        let workspace_root = self.workspace_to_current_dir(workspace);
        #[cfg_attr(feature = "qa-mode", allow(unused_variables))]
        if let Some(msg_store) = self.get_msg_store_by_id(&execution_process.id).await
            && let Some((executor_profile_id, working_dir)) = match executor_action.typ() {
                ExecutorActionType::CodingAgentInitialRequest(request) => Some((
                    request.executor_config.profile_id(),
                    request.effective_dir(&workspace_root),
                )),
                ExecutorActionType::CodingAgentFollowUpRequest(request) => Some((
                    request.executor_config.profile_id(),
                    request.effective_dir(&workspace_root),
                )),
                ExecutorActionType::ReviewRequest(request) => Some((
                    request.executor_config.profile_id(),
                    request.effective_dir(&workspace_root),
                )),
                _ => None,
            }
        {
            #[cfg(feature = "qa-mode")]
            {
                let executor = QaMockExecutor;
                let _ = executor.normalize_logs(msg_store, &working_dir);
            }
            #[cfg(not(feature = "qa-mode"))]
            {
                if let Some(executor) =
                    ExecutorConfigs::get_cached().get_coding_agent(&executor_profile_id)
                {
                    let _ = executor.normalize_logs(msg_store, &working_dir);
                } else {
                    tracing::error!(
                        "Failed to resolve profile '{:?}' for normalization",
                        executor_profile_id
                    );
                }
            }
        }

        // Detached persistent processes (unix) write their own raw log file,
        // which is the persistent record; mirroring the MsgStore into a JSONL
        // file would duplicate it on every adoption replay.
        let writes_own_raw_log = cfg!(unix) && run_reason.is_persistent();
        if !writes_own_raw_log {
            execution_process::spawn_stream_raw_logs_to_storage(
                self.msg_stores().clone(),
                self.db().clone(),
                execution_process.id,
                session.id,
            );
        }

        // Reset the reported pipeline stage only when a *new coding-agent*
        // execution begins (not for setup/cleanup/archive/dev-server runs,
        // which would otherwise wrongly wipe a live stage). The tracker
        // spawned below will repopulate it as the fresh execution reports
        // markers.
        if *run_reason == ExecutionProcessRunReason::CodingAgent
            && let Err(e) =
                Workspace::set_current_pipeline_stage(&self.db().pool, workspace.id, None).await
        {
            tracing::warn!(
                "Failed to reset current_pipeline_stage for workspace {}: {}",
                workspace.id,
                e
            );
        }

        // Provision the SpecKit scaffold for SpecKit workspaces. Durable gate
        // first (`speckit_feature_key` set — covers follow-ups after the first
        // provisioning), else the tightened prompt gate (a composed
        // `## Pipeline` block that names a `/speckit.` command). Provisioning
        // failure must never block the execution: warn and continue.
        if *run_reason == ExecutionProcessRunReason::CodingAgent {
            let prompt = match executor_action.typ() {
                ExecutorActionType::CodingAgentInitialRequest(request) => {
                    Some(request.prompt.as_str())
                }
                ExecutorActionType::CodingAgentFollowUpRequest(request) => {
                    Some(request.prompt.as_str())
                }
                _ => None,
            };
            let speckit_enabled = workspace.speckit_feature_key.is_some()
                || prompt.is_some_and(crate::services::speckit::is_speckit_pipeline);
            if speckit_enabled
                && let Err(e) =
                    crate::services::speckit::provision_workspace(&self.db().pool, workspace).await
            {
                tracing::warn!(
                    "Failed to provision SpecKit scaffold for workspace {}: {}",
                    workspace.id,
                    e
                );
            }
        }

        if *run_reason == ExecutionProcessRunReason::CodingAgent
            && let Some(store) = self.get_msg_store_by_id(&execution_process.id).await
        {
            crate::services::pipeline_stage::spawn_pipeline_stage_tracker(
                store,
                workspace.id,
                execution_process.id,
                self.db().clone(),
            );
        }

        Ok(execution_process)
    }

    async fn try_start_next_action(&self, ctx: &ExecutionContext) -> Result<(), ContainerError> {
        let action = ctx.execution_process.executor_action()?;
        let next_action = if let Some(next_action) = action.next_action() {
            next_action
        } else {
            tracing::debug!("No next action configured");
            return Ok(());
        };

        // Determine the run reason of the next action
        let next_run_reason = match (action.typ(), next_action.typ()) {
            (ExecutorActionType::ScriptRequest(_), ExecutorActionType::ScriptRequest(_)) => {
                ExecutionProcessRunReason::SetupScript
            }
            (
                ExecutorActionType::CodingAgentInitialRequest(_)
                | ExecutorActionType::CodingAgentFollowUpRequest(_)
                | ExecutorActionType::ReviewRequest(_),
                ExecutorActionType::ScriptRequest(_),
            ) => ExecutionProcessRunReason::CleanupScript,
            (
                _,
                ExecutorActionType::CodingAgentFollowUpRequest(_)
                | ExecutorActionType::CodingAgentInitialRequest(_)
                | ExecutorActionType::ReviewRequest(_),
            ) => ExecutionProcessRunReason::CodingAgent,
        };

        self.start_execution(&ctx.workspace, &ctx.session, next_action, &next_run_reason)
            .await?;

        tracing::debug!("Started next action: {:?}", next_action);
        Ok(())
    }
}

/// Prepend project-scoping context to an initial coding-agent prompt.
///
/// When a workspace targets a subdirectory of a single repository (for example,
/// one service inside a shared homelab monorepo), several projects map to the
/// same repo and the agent is started inside that subdirectory. Nothing in the
/// prompt otherwise tells the agent which project it is working on, so add a
/// short note describing the working directory and asking it to keep changes
/// scoped there. Prompts for multi-repo or whole-repo workspaces are returned
/// unchanged.
fn scope_initial_prompt_to_working_dir(prompt: String, repos: &[Repo]) -> String {
    let [repo] = repos else {
        return prompt;
    };

    let Some(subdir) = repo
        .default_working_dir
        .as_deref()
        .map(str::trim)
        .filter(|subdir| !subdir.is_empty())
    else {
        return prompt;
    };

    let repo_name = &repo.display_name;
    format!(
        "You are working in the `{subdir}` directory of the `{repo_name}` \
         repository, which is shared by multiple projects. This directory is \
         your current working directory and the root of the project for this \
         task—keep your changes scoped to it unless the task explicitly \
         requires touching other parts of the repository.\n\n{prompt}"
    )
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{Arc, atomic::AtomicBool},
    };

    use chrono::Utc;
    use db::models::repo::Repo;
    use executors::logs::utils::ConversationPatch;
    use futures::StreamExt;
    use serde_json::json;
    use tokio::{sync::Semaphore, time::Duration};
    use uuid::Uuid;

    use super::{
        HistoricalNormalizationLifetime, HistoricalNormalizationRegistry, LogMsg,
        is_indexed_entry_patch, replay_materialized_log, reset_would_discard_uncommitted_work,
        scope_initial_prompt_to_working_dir,
    };

    #[test]
    fn indexed_entry_patches_are_kept_and_repo_diff_patches_are_not() {
        let entry_patch = ConversationPatch::add_stdout(3, "hello".to_string());
        assert!(is_indexed_entry_patch(&entry_patch));

        let diff_patch = ConversationPatch::add_repo_diff(
            "repo",
            "src/main.rs",
            utils::diff::Diff {
                change: utils::diff::DiffChangeKind::Added,
                old_path: None,
                new_path: Some("src/main.rs".to_string()),
                old_content: None,
                new_content: None,
                content_omitted: false,
                additions: None,
                deletions: None,
                repo_id: None,
            },
        );
        assert!(!is_indexed_entry_patch(&diff_patch));

        let removed_diff_patch = ConversationPatch::remove_diff("repo/src/main.rs".to_string());
        assert!(!is_indexed_entry_patch(&removed_diff_patch));
    }

    #[tokio::test]
    async fn dropping_historical_normalization_aborts_its_tasks() {
        let permit = Arc::new(Semaphore::new(1)).acquire_owned().await.unwrap();
        let task = tokio::spawn(std::future::pending::<()>());
        let lifetime = HistoricalNormalizationLifetime {
            _permit: permit,
            _lease: HistoricalNormalizationRegistry::default()
                .acquire(Uuid::new_v4())
                .await,
            tasks: vec![task.abort_handle()],
            execution_id: Uuid::new_v4(),
            completed: Arc::new(AtomicBool::new(false)),
        };

        drop(lifetime);

        assert!(task.await.unwrap_err().is_cancelled());
    }

    #[tokio::test]
    async fn historical_normalization_is_single_flight_per_execution() {
        let registry = Arc::new(HistoricalNormalizationRegistry::default());
        let execution_id = Uuid::new_v4();
        let leader = registry.acquire(execution_id).await;
        assert!(!leader.joined_existing);

        let waiter_registry = registry.clone();
        let waiter = tokio::spawn(async move { waiter_registry.acquire(execution_id).await });
        tokio::task::yield_now().await;
        assert!(!waiter.is_finished());

        drop(leader);
        let waiter = tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .unwrap()
            .unwrap();
        assert!(waiter.joined_existing);
    }

    #[tokio::test]
    async fn historical_normalization_does_not_serialize_different_executions() {
        let registry = HistoricalNormalizationRegistry::default();
        let _first = registry.acquire(Uuid::new_v4()).await;

        tokio::time::timeout(Duration::from_millis(100), registry.acquire(Uuid::new_v4()))
            .await
            .expect("a different execution must acquire independently");
    }

    #[tokio::test]
    async fn canceled_waiter_does_not_release_the_leader() {
        let registry = Arc::new(HistoricalNormalizationRegistry::default());
        let execution_id = Uuid::new_v4();
        let leader = registry.acquire(execution_id).await;

        let waiter_registry = registry.clone();
        let waiter = tokio::spawn(async move { waiter_registry.acquire(execution_id).await });
        tokio::task::yield_now().await;
        waiter.abort();
        assert!(matches!(waiter.await, Err(error) if error.is_cancelled()));

        let retry_registry = registry.clone();
        let retry = tokio::spawn(async move { retry_registry.acquire(execution_id).await });
        tokio::task::yield_now().await;
        assert!(!retry.is_finished());
        drop(leader);
        assert!(
            tokio::time::timeout(Duration::from_secs(1), retry)
                .await
                .unwrap()
                .unwrap()
                .joined_existing
        );
    }

    #[tokio::test]
    async fn abandoned_leader_allows_a_waiter_to_retry() {
        let registry = Arc::new(HistoricalNormalizationRegistry::default());
        let execution_id = Uuid::new_v4();
        let leader = registry.acquire(execution_id).await;
        let waiter_registry = registry.clone();
        let waiter = tokio::spawn(async move { waiter_registry.acquire(execution_id).await });
        tokio::task::yield_now().await;

        drop(leader);

        tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn dead_historical_normalization_cells_are_reclaimed() {
        let registry = HistoricalNormalizationRegistry::default();
        let first_id = Uuid::new_v4();
        drop(registry.acquire(first_id).await);

        let _second = registry.acquire(Uuid::new_v4()).await;
        let cells = registry.cells.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(cells.len(), 1);
        assert!(!cells.contains_key(&first_id));
    }

    #[tokio::test]
    async fn waiter_replays_the_sidecar_published_by_the_leader() {
        let registry = Arc::new(HistoricalNormalizationRegistry::default());
        let execution_id = Uuid::new_v4();
        let leader = registry.acquire(execution_id).await;
        let dir = tempfile::tempdir().unwrap();
        let cache_path = dir.path().join("execution.normalized.jsonl");

        let waiter_registry = registry.clone();
        let waiter_path = cache_path.clone();
        let waiter = tokio::spawn(async move {
            let _lease = waiter_registry.acquire(execution_id).await;
            replay_materialized_log(&waiter_path, execution_id, "test_after_wait").await
        });
        tokio::task::yield_now().await;
        assert!(!waiter.is_finished());

        let entries = vec![json!({"type": "NORMALIZED_ENTRY", "content": "complete"})];
        super::normalized_log_cache::write(
            &cache_path,
            super::normalized_log_cache::CacheHeader {
                version: super::normalized_log_cache::CACHE_VERSION,
                entry_count: entries.len(),
                truncated: false,
            },
            &entries,
        )
        .await
        .unwrap();
        drop(leader);

        let mut stream = waiter.await.unwrap().expect("waiter should replay cache");
        let mut patches = Vec::new();
        while let Some(message) = stream.next().await {
            if let LogMsg::JsonPatch(patch) = message.unwrap() {
                patches.push(patch);
            }
        }
        assert_eq!(
            super::normalized_log_cache::materialize_entries(&patches).unwrap(),
            entries
        );
    }

    #[tokio::test]
    async fn valid_materialized_log_is_immediately_replayable() {
        let execution_id = Uuid::new_v4();
        let dir = tempfile::tempdir().unwrap();
        let cache_path = dir.path().join("execution.normalized.jsonl");
        let entries = vec![json!("cached")];
        super::normalized_log_cache::write(
            &cache_path,
            super::normalized_log_cache::CacheHeader {
                version: super::normalized_log_cache::CACHE_VERSION,
                entry_count: entries.len(),
                truncated: false,
            },
            &entries,
        )
        .await
        .unwrap();

        let started = std::time::Instant::now();
        let mut stream = replay_materialized_log(&cache_path, execution_id, "test_optimistic")
            .await
            .unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(matches!(
            stream.next().await.unwrap().unwrap(),
            LogMsg::JsonPatch(_)
        ));
    }

    #[test]
    fn dirty_git_reset_requires_explicit_force() {
        assert!(reset_would_discard_uncommitted_work(true, true, false));
        assert!(!reset_would_discard_uncommitted_work(true, true, true));
        assert!(!reset_would_discard_uncommitted_work(true, false, false));
        assert!(!reset_would_discard_uncommitted_work(false, true, false));
    }

    fn repo_with_working_dir(display_name: &str, working_dir: Option<&str>) -> Repo {
        Repo {
            id: Uuid::new_v4(),
            path: PathBuf::from("/tmp/repo"),
            name: display_name.to_string(),
            display_name: display_name.to_string(),
            setup_script: None,
            cleanup_script: None,
            archive_script: None,
            copy_files: None,
            parallel_setup_script: false,
            dev_server_script: None,
            default_target_branch: None,
            default_working_dir: working_dir.map(str::to_string),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn prepends_context_when_single_repo_targets_a_subdirectory() {
        let repos = vec![repo_with_working_dir("homelab", Some("services/grafana"))];
        let result = scope_initial_prompt_to_working_dir("Bump the image tag".to_string(), &repos);

        assert!(result.contains("`services/grafana`"));
        assert!(result.contains("`homelab`"));
        assert!(result.ends_with("Bump the image tag"));
    }

    #[test]
    fn leaves_prompt_unchanged_without_a_working_dir() {
        let repos = vec![repo_with_working_dir("homelab", None)];
        let result = scope_initial_prompt_to_working_dir("Do the thing".to_string(), &repos);

        assert_eq!(result, "Do the thing");
    }

    #[test]
    fn leaves_prompt_unchanged_for_empty_working_dir() {
        let repos = vec![repo_with_working_dir("homelab", Some("   "))];
        let result = scope_initial_prompt_to_working_dir("Do the thing".to_string(), &repos);

        assert_eq!(result, "Do the thing");
    }

    #[test]
    fn leaves_prompt_unchanged_for_multi_repo_workspaces() {
        let repos = vec![
            repo_with_working_dir("homelab", Some("services/grafana")),
            repo_with_working_dir("infra", Some("modules/dns")),
        ];
        let result = scope_initial_prompt_to_working_dir("Do the thing".to_string(), &repos);

        assert_eq!(result, "Do the thing");
    }

    mod auto_resume {
        use executors::{
            actions::{
                ExecutorAction, ExecutorActionType,
                coding_agent_follow_up::CodingAgentFollowUpRequest,
                coding_agent_initial::CodingAgentInitialRequest,
                script::{ScriptContext, ScriptRequest, ScriptRequestLanguage},
            },
            executors::BaseCodingAgent,
            profile::ExecutorConfig,
        };

        use crate::services::container::{
            RESUME_INTERRUPTED_PROMPT, executor_config_for_auto_resume,
        };

        fn executor_config() -> ExecutorConfig {
            ExecutorConfig::new(BaseCodingAgent::ClaudeCode)
        }

        fn follow_up_action(prompt: &str) -> ExecutorAction {
            ExecutorAction::new(
                ExecutorActionType::CodingAgentFollowUpRequest(CodingAgentFollowUpRequest {
                    prompt: prompt.to_string(),
                    session_id: "agent-session".to_string(),
                    reset_to_message_id: None,
                    executor_config: executor_config(),
                    working_dir: None,
                }),
                None,
            )
        }

        #[test]
        fn resumes_interrupted_initial_requests() {
            let action = ExecutorAction::new(
                ExecutorActionType::CodingAgentInitialRequest(CodingAgentInitialRequest {
                    prompt: "Build the feature".to_string(),
                    executor_config: executor_config(),
                    working_dir: None,
                }),
                None,
            );

            assert!(executor_config_for_auto_resume(&action).is_some());
        }

        #[test]
        fn resumes_interrupted_user_follow_ups() {
            let action = follow_up_action("Please also add tests");

            assert!(executor_config_for_auto_resume(&action).is_some());
        }

        #[test]
        fn does_not_resume_a_run_twice() {
            // A run whose prompt is the resume prompt was already resumed
            // once; skipping it caps auto-resume in a crash-restart loop.
            let action = follow_up_action(RESUME_INTERRUPTED_PROMPT);

            assert!(executor_config_for_auto_resume(&action).is_none());
        }

        #[test]
        fn does_not_resume_non_coding_agent_actions() {
            let action = ExecutorAction::new(
                ExecutorActionType::ScriptRequest(ScriptRequest {
                    script: "echo hi".to_string(),
                    language: ScriptRequestLanguage::Bash,
                    context: ScriptContext::SetupScript,
                    working_dir: None,
                }),
                None,
            );

            assert!(executor_config_for_auto_resume(&action).is_none());
        }
    }
}
