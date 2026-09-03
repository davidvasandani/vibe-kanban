use std::{
    collections::{BTreeMap, HashMap},
    io,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::anyhow;
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use chrono::{DateTime, Utc};
use cluster_protocol::{
    CancellationPhase, CancellationRequest, EventAcknowledgement, ExecutionDispatch,
    ExecutionEventPayload, InteractionRequest, InteractionResponse, McpConfigSnapshot,
    McpRefreshRequest, PROTOCOL_VERSION, PersistencePolicy, RequestAuthority, TerminalState,
    WorkerMcpRefreshStatus,
};
use command_group::AsyncGroupChild;
use db::{
    DBService,
    models::{
        coding_agent_turn::CodingAgentTurn,
        execution_process::{
            ExecutionContext, ExecutionProcess, ExecutionProcessRunReason, ExecutionProcessStatus,
            ExecutorActionField,
        },
        execution_process_repo_state::ExecutionProcessRepoState,
        execution_worker_job::{ExecutionWorkerDispatchState, ExecutionWorkerJob},
        project::Project,
        repo::Repo,
        scratch::{DraftFollowUpData, Scratch, ScratchType},
        session::{Session, SessionError},
        task::Task,
        worker_node::{WorkerMountStatus, WorkerNode, WorkerNodeStatus},
        workspace::{Workspace, WorkspacePlacement, WorkspacePlacementState},
        workspace_repo::WorkspaceRepo,
    },
};
use deployment::DeploymentError;
use executors::{
    actions::{
        Executable, ExecutorAction, ExecutorActionType,
        coding_agent_follow_up::CodingAgentFollowUpRequest,
        coding_agent_initial::CodingAgentInitialRequest,
    },
    approvals::{ExecutorApprovalService, NoopExecutorApprovalService},
    env::{ExecutionEnv, RepoContext},
    executors::{
        BaseCodingAgent, CancellationToken, ExecutorExitResult, ExecutorExitSignal,
        WarmReuseHandle, WarmReuseSignal,
    },
    logs::{NormalizedEntryType, utils::patch::extract_normalized_entry_from_patch},
    mcp_config::read_coding_agent_mcp_servers,
    mcp_refresh::{
        McpRefreshErrorCategory, McpRefreshHandle, McpRefreshResult, McpRefreshSignal,
        McpRefreshStatus,
    },
    profile::{ExecutorConfig, ExecutorConfigs, ExecutorProfile},
};
use futures::{FutureExt, StreamExt, TryStreamExt, stream::select};
use git::GitService;
use serde_json::json;
use services::services::{
    analytics::AnalyticsContext,
    approvals::{Approvals, executor_approvals::ExecutorApprovalBridge},
    cluster::{ClusterConfig, WorkerClient},
    config::{Config, DEFAULT_COMMIT_REMINDER_PROMPT},
    container::{ContainerError, ContainerRef, ContainerService},
    diff_stream::{self, DiffStreamHandle},
    file::FileService,
    mcp_refresh::McpRefreshCoordinator,
    notification::NotificationService,
    queued_message::QueuedMessageService,
    remote_client::{RemoteClient, RemoteClientError},
    remote_sync,
};
use sha2::{Digest, Sha256};
use tokio::{sync::RwLock, task::JoinHandle};
use tokio_util::io::ReaderStream;
use utils::{
    approvals::{ApprovalOutcome, ApprovalRequest},
    log_msg::LogMsg,
    msg_store::MsgStore,
    text::{git_branch_id, short_uuid, truncate_to_char_boundary},
    worktree_linkage::WorktreeLinkage,
};
use uuid::Uuid;
use workspace_manager::{
    AdoptOutcome, RepoWorkspaceInput, SharedRepositoryStore, SharedWorkspacePaths, WorkspaceError,
    WorkspaceManager,
};
use worktree_manager::RepositoryAdminLockManager;

use crate::{command, copy};

const WORKSPACE_TOUCH_DEBOUNCE: Duration = Duration::from_mins(2);
const FINAL_OUTPUT_RECONCILIATION_TIMEOUT: Duration = Duration::from_secs(45);
type AsyncChildStore = Arc<RwLock<HashMap<Uuid, Arc<RwLock<AsyncGroupChild>>>>>;

fn history_has_final_assistant_message(history: &[LogMsg]) -> bool {
    history
        .iter()
        .rev()
        .find_map(normalized_final_assistant_state)
        .unwrap_or(false)
}

fn normalized_final_assistant_state(message: &LogMsg) -> Option<bool> {
    let LogMsg::JsonPatch(patch) = message else {
        return None;
    };
    let (_, entry) = extract_normalized_entry_from_patch(patch)?;
    match entry.entry_type {
        NormalizedEntryType::AssistantMessage => Some(!entry.content.trim().is_empty()),
        // Accounting/progress entries can legitimately trail the final
        // assistant entry without representing more agent work.
        NormalizedEntryType::TokenUsageInfo(_)
        | NormalizedEntryType::Loading
        | NormalizedEntryType::Thinking => None,
        // A tool request, interaction, error, or another conversational entry
        // is positive evidence that earlier assistant output was intermediate.
        _ => Some(false),
    }
}

async fn wait_for_unfinalized_output(
    msg_stores: Arc<RwLock<HashMap<Uuid, Arc<MsgStore>>>>,
    child_store: Option<AsyncChildStore>,
    exec_id: Uuid,
    live_child_is_turn_evidence: bool,
) {
    let mut observed_final_at = None;
    loop {
        let history = msg_stores
            .read()
            .await
            .get(&exec_id)
            .map(|store| store.get_history())
            .unwrap_or_default();
        if history_has_final_assistant_message(&history) {
            // Preserve the first observation so a capped history that evicts
            // one entry for every append cannot postpone reconciliation merely
            // by keeping the same retained length.
            observed_final_at.get_or_insert_with(tokio::time::Instant::now);
        } else {
            observed_final_at = None;
        }
        if observed_final_at.is_some_and(|observed| {
            tokio::time::Instant::now().duration_since(observed)
                >= FINAL_OUTPUT_RECONCILIATION_TIMEOUT
        }) {
            if live_child_is_turn_evidence
                && let Some(child_store) = &child_store
                && let Some(child) = child_store.read().await.get(&exec_id).cloned()
                && matches!(child.write().await.try_wait(), Ok(None))
            {
                // A live child is authoritative positive liveness. Re-arm the
                // observation window; process exit or executor completion owns
                // terminal classification while that evidence remains true.
                observed_final_at = Some(tokio::time::Instant::now());
                continue;
            }
            return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn update_completion_with_retry(
    db: &DBService,
    exec_id: Uuid,
    status: ExecutionProcessStatus,
    exit_code: Option<i64>,
) -> Result<(), anyhow::Error> {
    let mut delay = Duration::from_millis(50);
    for attempt in 1..=3 {
        match ExecutionProcess::update_completion(&db.pool, exec_id, status.clone(), exit_code)
            .await
        {
            Ok(()) => return Ok(()),
            Err(error) if attempt < 3 => {
                tracing::warn!(%exec_id, ?status, attempt, %error, "Retrying execution completion write");
                tokio::time::sleep(delay).await;
                delay *= 2;
            }
            Err(error) => return Err(error.into()),
        }
    }
    unreachable!("bounded completion attempts always return")
}

fn should_ack_worker_batch(cursor: u64, has_terminal: bool) -> bool {
    cursor > 0 && !has_terminal
}

fn worker_job_has_positive_liveness(
    job: &ExecutionWorkerJob,
    now: DateTime<Utc>,
    live_lease_is_turn_evidence: bool,
) -> bool {
    live_lease_is_turn_evidence
        && !job.dispatch_state.is_terminal()
        && job.lease_expires_at.is_some_and(|lease| lease > now)
}

fn worker_lease_is_turn_evidence(base_executor: Option<BaseCodingAgent>) -> bool {
    // Keep this aligned with executors whose SpawnedChild carries
    // `exit_signal: Some(_)`. Their worker process can stay alive independently
    // of the protocol turn, so a renewed lease is not evidence of turn work.
    !matches!(
        base_executor,
        Some(
            BaseCodingAgent::Codex
                | BaseCodingAgent::Opencode
                | BaseCodingAgent::Gemini
                | BaseCodingAgent::QwenCode
                | BaseCodingAgent::Copilot
                | BaseCodingAgent::Grok
        )
    )
}

#[cfg(test)]
mod final_output_reconciliation_tests {
    use std::{collections::HashMap, sync::Arc, time::Duration};

    use command_group::AsyncCommandGroup;
    use executors::logs::{NormalizedEntry, NormalizedEntryType, utils::patch::ConversationPatch};
    use tokio::sync::RwLock;
    use utils::{log_msg::LogMsg, msg_store::MsgStore};
    use uuid::Uuid;

    use super::{
        history_has_final_assistant_message, normalized_final_assistant_state,
        should_ack_worker_batch, wait_for_unfinalized_output, worker_job_has_positive_liveness,
        worker_lease_is_turn_evidence,
    };

    fn normalized_message(entry_type: NormalizedEntryType, content: &str) -> LogMsg {
        LogMsg::JsonPatch(ConversationPatch::add_normalized_entry(
            0,
            NormalizedEntry {
                timestamp: None,
                entry_type,
                content: content.into(),
                metadata: None,
            },
        ))
    }

    #[test]
    fn final_assistant_output_arms_reconciliation_but_empty_output_does_not() {
        let final_message =
            normalized_message(NormalizedEntryType::AssistantMessage, "Final answer");
        let empty_message = normalized_message(NormalizedEntryType::AssistantMessage, "  ");
        let tool_after_output = normalized_message(
            NormalizedEntryType::ToolUse {
                tool_name: "shell".into(),
                action_type: executors::logs::ActionType::CommandRun {
                    command: "sleep 60".into(),
                    result: None,
                    category: Default::default(),
                },
                status: executors::logs::ToolStatus::Created,
            },
            "pending",
        );

        assert!(history_has_final_assistant_message(std::slice::from_ref(
            &final_message
        )));
        assert!(!history_has_final_assistant_message(&[
            final_message,
            tool_after_output.clone(),
        ]));
        assert!(!history_has_final_assistant_message(&[empty_message]));
        assert!(!history_has_final_assistant_message(&[LogMsg::Stdout(
            "Final answer".into()
        )]));
        assert_eq!(
            normalized_final_assistant_state(&tool_after_output),
            Some(false)
        );
    }

    #[test]
    fn terminal_worker_batch_is_not_acknowledged_before_persistence() {
        assert!(should_ack_worker_batch(4, false));
        assert!(!should_ack_worker_batch(4, true));
        assert!(!should_ack_worker_batch(0, false));
    }

    #[test]
    fn worker_job_liveness_requires_active_state_and_current_lease() {
        let now = chrono::Utc::now();
        let mut job = db::models::execution_worker_job::ExecutionWorkerJob {
            execution_process_id: Uuid::new_v4(),
            worker_node_id: Uuid::new_v4(),
            worker_job_id: Uuid::new_v4(),
            request_digest: "digest".into(),
            dispatch_state: db::models::execution_worker_job::ExecutionWorkerDispatchState::Running,
            last_event_sequence: 0,
            worker_last_sequence: 0,
            lease_expires_at: Some(now + chrono::Duration::seconds(30)),
            output_complete: false,
            terminal_evidence: None,
            dispatched_at: now,
            accepted_at: Some(now),
            completed_at: None,
            created_at: now,
            updated_at: now,
        };
        assert!(worker_job_has_positive_liveness(&job, now, true));
        assert!(
            !worker_job_has_positive_liveness(&job, now, false),
            "a signal-driven worker lease is process liveness, not turn liveness"
        );

        job.lease_expires_at = Some(now - chrono::Duration::seconds(1));
        assert!(!worker_job_has_positive_liveness(&job, now, true));
        job.lease_expires_at = Some(now + chrono::Duration::seconds(30));
        job.dispatch_state =
            db::models::execution_worker_job::ExecutionWorkerDispatchState::Completed;
        assert!(!worker_job_has_positive_liveness(&job, now, true));
    }

    #[test]
    fn signal_driven_worker_lease_is_not_turn_liveness() {
        use executors::executors::BaseCodingAgent;

        for executor in [
            BaseCodingAgent::Codex,
            BaseCodingAgent::Opencode,
            BaseCodingAgent::Gemini,
            BaseCodingAgent::QwenCode,
            BaseCodingAgent::Copilot,
            BaseCodingAgent::Grok,
        ] {
            assert!(!worker_lease_is_turn_evidence(Some(executor)));
        }
        for executor in [
            BaseCodingAgent::ClaudeCode,
            BaseCodingAgent::Amp,
            BaseCodingAgent::CursorAgent,
            BaseCodingAgent::Droid,
        ] {
            assert!(worker_lease_is_turn_evidence(Some(executor)));
        }
        assert!(worker_lease_is_turn_evidence(None));
    }

    #[tokio::test(start_paused = true)]
    async fn final_output_without_terminal_evidence_triggers_bounded_reconciliation() {
        let execution_id = Uuid::new_v4();
        let store = Arc::new(MsgStore::new());
        store.push(normalized_message(
            NormalizedEntryType::AssistantMessage,
            "Final answer",
        ));
        let stores = Arc::new(RwLock::new(HashMap::from([(execution_id, store)])));

        let reconciliation = tokio::spawn(wait_for_unfinalized_output(
            stores,
            None,
            execution_id,
            true,
        ));
        tokio::time::advance(Duration::from_secs(44)).await;
        assert!(!reconciliation.is_finished());
        tokio::time::advance(Duration::from_secs(2)).await;
        reconciliation.await.expect("reconciliation task completes");
    }

    #[tokio::test(start_paused = true)]
    async fn positive_local_process_liveness_defers_reconciliation() {
        let execution_id = Uuid::new_v4();
        let store = Arc::new(MsgStore::new());
        store.push(normalized_message(
            NormalizedEntryType::AssistantMessage,
            "Final answer",
        ));
        let stores = Arc::new(RwLock::new(HashMap::from([(execution_id, store)])));
        let child = Arc::new(RwLock::new(
            tokio::process::Command::new("sleep")
                .arg("300")
                .group_spawn()
                .expect("spawn live child"),
        ));
        let children = Arc::new(RwLock::new(HashMap::from([(execution_id, child.clone())])));

        let reconciliation = tokio::spawn(wait_for_unfinalized_output(
            stores,
            Some(children),
            execution_id,
            true,
        ));
        tokio::time::advance(Duration::from_secs(46)).await;
        assert!(!reconciliation.is_finished());

        child.write().await.kill().await.expect("kill child");
        tokio::time::advance(Duration::from_secs(46)).await;
        reconciliation
            .await
            .expect("dead child permits reconciliation");
    }

    #[tokio::test(start_paused = true)]
    async fn signal_driven_turn_does_not_treat_live_child_as_turn_liveness() {
        let execution_id = Uuid::new_v4();
        let store = Arc::new(MsgStore::new());
        store.push(normalized_message(
            NormalizedEntryType::AssistantMessage,
            "Final answer",
        ));
        let stores = Arc::new(RwLock::new(HashMap::from([(execution_id, store)])));
        let child = Arc::new(RwLock::new(
            tokio::process::Command::new("sleep")
                .arg("300")
                .group_spawn()
                .expect("spawn live app-server child"),
        ));
        let children = Arc::new(RwLock::new(HashMap::from([(execution_id, child.clone())])));

        let reconciliation = tokio::spawn(wait_for_unfinalized_output(
            stores,
            Some(children),
            execution_id,
            false,
        ));
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(46)).await;
        tokio::task::yield_now().await;
        assert!(
            reconciliation.is_finished(),
            "an app-server child can outlive its turn and must not defer forever"
        );

        child.write().await.kill().await.expect("kill child");
        reconciliation.await.expect("reconciliation task completes");
    }
}

/// Env gate for warm coding-agent process reuse (Phase 2). Default off: until
/// this is set truthy, no app-server is kept warm and the runtime behaves
/// exactly as before (Constitution IV — do not enable a runtime path we cannot
/// observe E2E here). See `specs/vk/826e-coding-agent-war/`.
const KEEP_WARM_ENV: &str = "VK_KEEP_WARM_AGENTS";

fn validated_mcp_snapshot(
    executor: BaseCodingAgent,
    mut servers: BTreeMap<String, serde_json::Value>,
) -> anyhow::Result<McpConfigSnapshot> {
    executors::mcp_config::route_mcp_servers_for_runtime(&mut servers);
    let snapshot = McpConfigSnapshot {
        executor: executor.to_string(),
        servers,
    };
    snapshot.validate_size().map_err(anyhow::Error::from)?;
    Ok(snapshot)
}

fn push_worker_bytes(store: &MsgStore, encoded: &str, stderr: bool) {
    let message = match BASE64_STANDARD.decode(encoded) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(error) => {
            store.push(LogMsg::Stderr(format!(
                "Worker returned invalid base64 output: {error}"
            )));
            return;
        }
    };
    if stderr {
        store.push(LogMsg::Stderr(message));
    } else {
        store.push_stdout(message);
    }
}

async fn mark_remote_execution_indeterminate(
    db: &DBService,
    execution_id: Uuid,
) -> Result<(), ContainerError> {
    ExecutionWorkerJob::update_state(
        &db.pool,
        execution_id,
        ExecutionWorkerDispatchState::Indeterminate,
        None,
        Some(Utc::now()),
    )
    .await?;
    ExecutionProcess::update_completion(
        &db.pool,
        execution_id,
        ExecutionProcessStatus::Indeterminate,
        None,
    )
    .await?;
    Ok(())
}

fn worker_cleanup_evidence_safe(
    worker: &WorkerNode,
    has_unsafe_jobs: bool,
    now: DateTime<Utc>,
) -> bool {
    worker.status == WorkerNodeStatus::Online
        && worker.mount_status == WorkerMountStatus::Healthy
        && worker.lease_expires_at.is_some_and(|lease| lease > now)
        && !has_unsafe_jobs
}

/// A warm app-server with no active turn for longer than this is proactively
/// reaped so an abandoned-but-not-closed attempt cannot pin a process forever
/// (spec FR-5). The periodic workspace-cleanup sweep enforces it.
const WARM_IDLE_TIMEOUT: Duration = Duration::from_mins(30);

#[derive(Debug, PartialEq, Eq)]
enum SkippedCleanupAction {
    StartQueuedFollowUp,
    Finalize,
}

fn skipped_cleanup_action(has_queued_message: bool) -> SkippedCleanupAction {
    if has_queued_message {
        SkippedCleanupAction::StartQueuedFollowUp
    } else {
        SkippedCleanupAction::Finalize
    }
}

/// A persistent app-server (e.g. OpenCode) kept alive between turns for reuse.
/// Owned by the container's `warm_app_servers` registry, which is the single
/// reaper of its lifetime (Constitution V) at attempt/workspace teardown, stop,
/// idle-timeout, out-of-band death, and shutdown. See
/// `specs/vk/826e-coding-agent-war/data-model.md`.
struct WarmAppServer {
    /// The live warm process, moved out of `child_store` on a clean warm turn end.
    child: Arc<RwLock<AsyncGroupChild>>,
    /// Process-group id (mirrors `ExecutionProcess.pgid`); informational — the
    /// `child` handle is what `kill_process_group` reaps. Kept for parity with
    /// the restart re-adoption path.
    #[allow(dead_code)]
    pgid: Option<i32>,
    /// Connection facts a follow-up turn uses to reach this server (base_url,
    /// password, session id).
    reuse: WarmReuseHandle,
    /// Last time a turn started/ended on this server; drives the idle reaper.
    last_active: Instant,
}

impl WarmAppServer {
    /// True while the underlying process has not exited. A dead entry must be
    /// reaped and treated as a reuse miss (spec FR-6).
    async fn is_alive(&self) -> bool {
        let mut child = self.child.write().await;
        matches!(child.try_wait(), Ok(None))
    }
}

/// The warm app-server registry: session id → warm entry. Kept as a free type +
/// free functions (below) so the reap/register/take/sweep logic is unit-testable
/// against a real child process without standing up a DB-backed container.
type WarmRegistry = RwLock<HashMap<Uuid, WarmAppServer>>;

/// Parse a keep-warm gate value. Truthy = `1`/`true`/`yes`/`on`
/// (case-insensitive, trimmed); everything else (incl. unset/empty) is off.
fn parse_keep_warm(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Whether the keep-warm env gate (`VK_KEEP_WARM_AGENTS`) is on. Default off
/// keeps runtime behavior byte-for-byte identical to today (spec FR-8).
fn keep_warm_env_enabled() -> bool {
    parse_keep_warm(&std::env::var(KEEP_WARM_ENV).unwrap_or_default())
}

/// Reap the warm entry for a session: kill its process group and drop the entry.
/// Idempotent. This is the owner that closes the leak where a `Completed` turn
/// row is skipped by the `Running`-only teardown (spec FR-4).
async fn reap_warm_entry(registry: &WarmRegistry, session_id: &Uuid) {
    // Remove under the map lock, then kill after releasing it (never hold the
    // registry lock across the child-kill await).
    let entry = registry.write().await.remove(session_id);
    if let Some(entry) = entry {
        let mut child = entry.child.write().await;
        if let Err(err) = command::kill_process_group(&mut child).await {
            tracing::warn!("Failed to reap warm app-server for session {session_id}: {err}");
        } else {
            tracing::info!("Reaped warm app-server for session {session_id}");
        }
    }
}

/// Reap a warm entry only if it is still the same generation observed earlier
/// (matched by `last_active`, which is set fresh on every (re)registration). This
/// prevents a deferred sweep from killing a warm server that was reaped and
/// re-registered in the window after it was inspected (a TOCTOU the plain
/// session-keyed reap would hit).
async fn reap_warm_entry_if_unchanged(
    registry: &WarmRegistry,
    session_id: &Uuid,
    expected_generation: Instant,
) {
    let entry = {
        let mut map = registry.write().await;
        match map.get(session_id) {
            Some(e) if e.last_active == expected_generation => map.remove(session_id),
            _ => None,
        }
    };
    if let Some(entry) = entry {
        let mut child = entry.child.write().await;
        let _ = command::kill_process_group(&mut child).await;
        tracing::info!("Reaped idle/dead warm app-server for session {session_id}");
    }
}

/// Insert a warm child into the registry, reaping any pre-existing entry for the
/// session — enforcing the at-most-one-warm-process-per-session invariant (spec
/// FR-1/FR-7). The replace is a single `insert` under one lock so two concurrent
/// same-session completions can't both see "no old entry" and silently orphan
/// one process; the displaced entry (if any) is reaped after the lock is dropped.
async fn register_warm_entry(
    registry: &WarmRegistry,
    session_id: Uuid,
    child: Arc<RwLock<AsyncGroupChild>>,
    reuse: WarmReuseHandle,
) {
    let pgid = child.read().await.id().map(|id| id as i32);
    let displaced = registry.write().await.insert(
        session_id,
        WarmAppServer {
            child,
            pgid,
            reuse,
            last_active: Instant::now(),
        },
    );
    if let Some(old) = displaced {
        let mut old_child = old.child.write().await;
        let _ = command::kill_process_group(&mut old_child).await;
        tracing::info!("Replaced + reaped prior warm app-server for session {session_id}");
    }
    tracing::info!("Registered warm app-server for session {session_id}");
}

/// Take a live warm entry's child + reuse handle, removing it. Returns the child
/// as an **owned** `AsyncGroupChild` so a follow-up can re-wrap it. `None` on a
/// miss, a died-out-of-band entry (reaped, spec FR-6), or the should-not-happen
/// still-shared case — all of which make the caller cold-start.
async fn take_live_warm_entry(
    registry: &WarmRegistry,
    session_id: &Uuid,
) -> Option<(AsyncGroupChild, WarmReuseHandle)> {
    let entry = registry.write().await.remove(session_id)?;
    if !entry.is_alive().await {
        let mut child = entry.child.write().await;
        let _ = command::kill_process_group(&mut child).await;
        tracing::info!("Warm app-server for session {session_id} died out-of-band; cold-starting");
        return None;
    }
    // Removed from the map, so the registry no longer holds a reference; the sweep
    // deliberately does not clone child handles, so the Arc is uniquely owned here
    // and unwraps to an owned child for the follow-up turn.
    match Arc::try_unwrap(entry.child) {
        Ok(lock) => Some((lock.into_inner(), entry.reuse)),
        Err(arc) => {
            // Should-not-happen: a transient extra reference. Do NOT kill a live
            // server — re-park it (unchanged generation) and report a miss so the
            // caller cold-starts; the next turn will find it again.
            registry.write().await.insert(
                *session_id,
                WarmAppServer {
                    child: arc,
                    pgid: entry.pgid,
                    reuse: entry.reuse,
                    last_active: entry.last_active,
                },
            );
            tracing::warn!(
                "Warm app-server child for session {session_id} transiently shared; re-parked, cold-starting this turn"
            );
            None
        }
    }
}

/// Pure staleness predicate: a warm entry last active at `last_active` is idle
/// once `timeout` has elapsed by `now`. Extracted so the idle rule is unit-tested
/// without depending on wall-clock `Instant::now()` (spec FR-5).
fn warm_entry_is_idle(last_active: Instant, now: Instant, timeout: Duration) -> bool {
    now.saturating_duration_since(last_active) >= timeout
}

/// Reap every warm entry whose process has died or has been idle past the
/// timeout (spec FR-5/FR-6). Level-triggered (Constitution VI).
///
/// Each stale candidate is tagged with its `last_active` generation while the
/// registry is read-locked, then reaped only if that generation is still current
/// (`reap_warm_entry_if_unchanged`) — so a server reaped-and-re-registered in the
/// interim is not killed. Liveness (`try_wait`) is a non-blocking poll and the
/// child locks here are uncontended (a parked warm child has no running turn), so
/// the brief read-lock hold does not stall registration/reuse; crucially the
/// sweep does **not** clone child handles, so it never makes `take_live_warm_entry`
/// see a shared Arc and cold-start a healthy server.
async fn sweep_idle_warm_entries(registry: &WarmRegistry) {
    let now = Instant::now();
    let stale: Vec<(Uuid, Instant)> = {
        let map = registry.read().await;
        let mut stale = Vec::new();
        for (session_id, entry) in map.iter() {
            let dead = !matches!(entry.child.write().await.try_wait(), Ok(None));
            if dead || warm_entry_is_idle(entry.last_active, now, WARM_IDLE_TIMEOUT) {
                stale.push((*session_id, entry.last_active));
            }
        }
        stale
    };
    for (session_id, generation) in stale {
        reap_warm_entry_if_unchanged(registry, &session_id, generation).await;
    }
}

/// Reap every warm entry (kill each group, drop all entries). Used on server
/// shutdown: a warm coding-agent process is a `Completed` row and is **not** in
/// the persistent-process boot re-adoption path, so it must be killed here or it
/// orphans across a restart (spec FR-4c).
async fn reap_all_warm_entries(registry: &WarmRegistry) {
    let entries: Vec<(Uuid, WarmAppServer)> = registry.write().await.drain().collect();
    for (session_id, entry) in entries {
        let mut child = entry.child.write().await;
        let _ = command::kill_process_group(&mut child).await;
        tracing::info!("Reaped warm app-server for session {session_id} on shutdown");
    }
}

/// The profile definition to dispatch alongside `config`: exactly the one
/// variant the request names, resolved against this coordinator's profiles.
///
/// One variant rather than the executor's whole profile, because that is all the
/// worker needs to run this job, and a dispatch is not a reason to copy the
/// operator's other configurations onto another host.
///
/// Overrides carried on the request are deliberately *not* applied here: the
/// worker applies them itself, from the action it already receives, so applying
/// them on both sides would double them.
///
/// `None` when the profile does not resolve here either. That is not this
/// function's failure to report — the coordinator refuses such a request through
/// its own resolution path, and inventing an error here would duplicate it.
fn dispatched_executor_profile(config: &ExecutorConfig) -> Option<ExecutorProfile> {
    let profile_id = config.profile_id();
    let variant = profile_id
        .variant
        .clone()
        .unwrap_or_else(|| "DEFAULT".into());
    let agent = ExecutorConfigs::get_cached().get_coding_agent(&profile_id)?;
    Some(ExecutorProfile {
        recently_used_models: None,
        configurations: HashMap::from([(variant, agent)]),
    })
}

#[derive(Clone)]
pub struct LocalContainerService {
    db: DBService,
    workspace_manager: WorkspaceManager,
    child_store: AsyncChildStore,
    cancellation_tokens: Arc<RwLock<HashMap<Uuid, CancellationToken>>>,
    msg_stores: Arc<RwLock<HashMap<Uuid, Arc<MsgStore>>>>,
    /// Tracks background tasks that stream logs to the database.
    /// When stopping execution, we await these to ensure logs are fully persisted.
    db_stream_handles: Arc<RwLock<HashMap<Uuid, JoinHandle<()>>>>,
    exit_monitor_handles: Arc<RwLock<HashMap<Uuid, JoinHandle<()>>>>,
    /// Tailer tasks streaming a detached process's raw log file into its
    /// MsgStore (dev servers write straight to a file instead of pipes).
    raw_log_tailers: Arc<RwLock<HashMap<Uuid, JoinHandle<()>>>>,
    /// Process group ids of dev servers adopted from a previous server
    /// instance. These have no child handle; they are managed by pgid.
    adopted_pgids: Arc<RwLock<HashMap<Uuid, i32>>>,
    /// Warm app-servers kept alive between turns, keyed by **session id** (one
    /// active agent session per attempt). This registry is the single owner of a
    /// warm process's lifetime — it reaps at teardown/stop/idle/death, closing
    /// the leak where a `Completed` turn row is skipped by `try_stop`. Phase 2,
    /// see `specs/vk/826e-coding-agent-war/`.
    warm_app_servers: Arc<RwLock<HashMap<Uuid, WarmAppServer>>>,
    mcp_refresh_controls: Arc<RwLock<HashMap<Uuid, (Uuid, McpRefreshHandle)>>>,
    mcp_refresh_coordinator: McpRefreshCoordinator,
    workspace_touch_times: Arc<RwLock<HashMap<Uuid, Instant>>>,
    config: Arc<RwLock<Config>>,
    git: GitService,
    file_service: FileService,
    analytics: Option<AnalyticsContext>,
    approvals: Approvals,
    queued_message_service: QueuedMessageService,
    notification_service: NotificationService,
    remote_client: Option<RemoteClient>,
    cluster_config: ClusterConfig,
    repository_admin_locks: RepositoryAdminLockManager,
    worker_client: Option<WorkerClient>,
}

impl LocalContainerService {
    fn route_worker_interaction(
        &self,
        execution_id: Uuid,
        worker_node_id: Uuid,
        interaction: InteractionRequest,
    ) {
        let Some(client) = self.worker_client.clone() else {
            tracing::error!(%execution_id, "Cannot route worker interaction without a worker client");
            return;
        };
        let Some(coordinator_id) = self.cluster_config.coordinator_id else {
            tracing::error!(%execution_id, "Cannot route worker interaction without coordinator identity");
            return;
        };
        let approvals = self.approvals.clone();
        tokio::spawn(async move {
            let mut request = ApprovalRequest::new(interaction.prompt, execution_id);
            request.id = interaction.interaction_id.to_string();
            if let Some(expires_at) = interaction.expires_at {
                request.timeout_at = expires_at;
            }
            let is_question = interaction.kind == "question";
            let Ok((_, waiter)) = approvals.create_with_waiter(request, is_question).await else {
                tracing::error!(%execution_id, interaction_id = %interaction.interaction_id, "Failed to register worker interaction");
                return;
            };
            let mut outcome = waiter.await;
            let deadline = interaction.expires_at;
            loop {
                if matches!(
                    interaction.disconnect_policy,
                    cluster_protocol::DisconnectPolicy::FailClosed
                ) && deadline.is_some_and(|deadline| Utc::now() >= deadline)
                {
                    outcome = ApprovalOutcome::Denied {
                        reason: Some(
                            "coordinator could not deliver approval before timeout".into(),
                        ),
                    };
                }
                let response = InteractionResponse {
                    authority: RequestAuthority {
                        protocol_version: PROTOCOL_VERSION,
                        coordinator_id,
                        worker_node_id,
                        correlation_id: execution_id,
                        issued_at: Utc::now(),
                        nonce: Uuid::new_v4().to_string(),
                    },
                    execution_id,
                    interaction_id: interaction.interaction_id,
                    response: serde_json::to_string(&outcome)
                        .expect("approval outcome must serialize"),
                };
                match client.respond_interaction(worker_node_id, &response).await {
                    Ok(()) => break,
                    Err(error) => {
                        tracing::warn!(%execution_id, interaction_id = %interaction.interaction_id, "Worker interaction response failed; retrying: {error}");
                        if matches!(
                            interaction.disconnect_policy,
                            cluster_protocol::DisconnectPolicy::Timeout
                        ) && deadline.is_some_and(|deadline| Utc::now() >= deadline)
                        {
                            outcome = ApprovalOutcome::TimedOut;
                        }
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });
    }

    fn register_mcp_refresh_control(
        &self,
        session_id: Uuid,
        execution_id: Uuid,
        execution_started_at: DateTime<Utc>,
        signal: McpRefreshSignal,
    ) {
        let controls = self.mcp_refresh_controls.clone();
        let coordinator = self.mcp_refresh_coordinator.clone();
        tokio::spawn(async move {
            let handle = match signal.await {
                Ok(handle) => handle,
                Err(_) => {
                    if coordinator
                        .status(session_id)
                        .await
                        .is_some_and(|state| state.status == McpRefreshStatus::PendingNextTurn)
                    {
                        coordinator
                            .fail(session_id, McpRefreshErrorCategory::InitializeFailed)
                            .await;
                    }
                    return;
                }
            };
            controls
                .write()
                .await
                .insert(session_id, (execution_id, handle.clone()));

            if let Some(state) = coordinator
                .status(session_id)
                .await
                .filter(|state| state.status == McpRefreshStatus::PendingNextTurn)
            {
                // A request racing this execution's startup cannot be
                // confirmed from this turn: its config was already resolved.
                // Queue it on this live thread and let the following turn
                // perform the atomic confirmation.
                if state.requested_at > execution_started_at {
                    if let Err(category) = handle.0.queue_refresh().await {
                        coordinator.fail(session_id, category).await;
                    }
                    return;
                }
                match handle.0.list_servers().await {
                    Ok(servers) => {
                        coordinator.confirm(session_id, servers).await;
                    }
                    Err(category) => {
                        coordinator.fail(session_id, category).await;
                    }
                }
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        db: DBService,
        workspace_manager: WorkspaceManager,
        msg_stores: Arc<RwLock<HashMap<Uuid, Arc<MsgStore>>>>,
        config: Arc<RwLock<Config>>,
        git: GitService,
        file_service: FileService,
        analytics: Option<AnalyticsContext>,
        approvals: Approvals,
        queued_message_service: QueuedMessageService,
        remote_client: Option<RemoteClient>,
        cluster_config: ClusterConfig,
        worker_client: Option<WorkerClient>,
    ) -> Self {
        let child_store = Arc::new(RwLock::new(HashMap::new()));
        let cancellation_tokens = Arc::new(RwLock::new(HashMap::new()));
        let db_stream_handles = Arc::new(RwLock::new(HashMap::new()));
        let exit_monitor_handles = Arc::new(RwLock::new(HashMap::new()));
        let raw_log_tailers = Arc::new(RwLock::new(HashMap::new()));
        let adopted_pgids = Arc::new(RwLock::new(HashMap::new()));
        let warm_app_servers = Arc::new(RwLock::new(HashMap::new()));
        let mcp_refresh_controls = Arc::new(RwLock::new(HashMap::new()));
        let workspace_touch_times = Arc::new(RwLock::new(HashMap::new()));
        let notification_service = NotificationService::new(config.clone());
        let repository_admin_locks =
            RepositoryAdminLockManager::new(db.pool.clone(), Duration::from_mins(5))
                .expect("static repository lock lease must be valid");

        LocalContainerService {
            db,
            workspace_manager,
            child_store,
            cancellation_tokens,
            msg_stores,
            db_stream_handles,
            exit_monitor_handles,
            raw_log_tailers,
            adopted_pgids,
            warm_app_servers,
            mcp_refresh_controls,
            mcp_refresh_coordinator: McpRefreshCoordinator::default(),
            workspace_touch_times,
            config,
            git,
            file_service,
            analytics,
            approvals,
            queued_message_service,
            notification_service,
            remote_client,
            cluster_config,
            repository_admin_locks,
            worker_client,
        }
    }

    pub fn start_cleanup_tasks(&self) {
        self.spawn_workspace_cleanup();
    }

    fn map_workspace_manager_error(err: WorkspaceError) -> ContainerError {
        match err {
            WorkspaceError::Database(err) => ContainerError::Sqlx(err),
            WorkspaceError::Worktree(err) => ContainerError::Worktree(err),
            WorkspaceError::GitService(err) => ContainerError::GitServiceError(err),
            WorkspaceError::Io(err) => ContainerError::Io(err),
            WorkspaceError::NoRepositories => {
                ContainerError::Other(anyhow!("No repositories provided"))
            }
            WorkspaceError::Repo(err) => ContainerError::Other(anyhow!(err)),
            WorkspaceError::WorkspaceNotFound => {
                ContainerError::Other(anyhow!("Workspace not found"))
            }
            WorkspaceError::RepoAlreadyAttached => {
                ContainerError::Other(anyhow!("Repository already attached to workspace"))
            }
            WorkspaceError::BranchNotFound { repo_name, branch } => ContainerError::Other(anyhow!(
                "Branch '{}' does not exist in repository '{}'",
                branch,
                repo_name
            )),
            WorkspaceError::PartialCreation(msg) => ContainerError::Other(anyhow!(msg)),
            WorkspaceError::WorktreeNotPortable { repo_name, detail } => {
                ContainerError::Other(anyhow!(
                    "Repository '{}' has a worktree other nodes cannot use: {}",
                    repo_name,
                    detail
                ))
            }
            // Kept out of `Other` on purpose: `Other` is rendered to the user as
            // a generic internal error, and this is the one provisioning failure
            // whose text is the diagnosis.
            err @ WorkspaceError::SharedStore { .. } => {
                ContainerError::SharedStore(err.to_string())
            }
            WorkspaceError::InvalidSharedRoot(path) => {
                ContainerError::Other(anyhow!("Invalid shared workspace root: {}", path.display()))
            }
        }
    }

    async fn workspace_repo_inputs(
        &self,
        workspace_id: Uuid,
    ) -> Result<(Vec<Repo>, Vec<RepoWorkspaceInput>), ContainerError> {
        let workspace_repos =
            WorkspaceRepo::find_by_workspace_id(&self.db.pool, workspace_id).await?;
        if workspace_repos.is_empty() {
            return Err(ContainerError::Other(anyhow!(
                "Workspace has no repositories configured"
            )));
        }

        let repositories =
            WorkspaceRepo::find_repos_for_workspace(&self.db.pool, workspace_id).await?;
        let target_branches: HashMap<_, _> = workspace_repos
            .iter()
            .map(|wr| (wr.repo_id, wr.target_branch.clone()))
            .collect();

        // Resolve, once and here, which Git directory administers this
        // workspace's worktrees. Every caller — create, ensure, cleanup, diff —
        // goes through this function, so the decision is made in one place
        // rather than re-derived (and eventually disagreed about) per call site.
        let store = self.shared_repository_store_for(workspace_id).await?;

        let workspace_inputs: Vec<RepoWorkspaceInput> = repositories
            .iter()
            .map(|repo| {
                let target_branch = target_branches.get(&repo.id).cloned().ok_or_else(|| {
                    ContainerError::Other(anyhow!(
                        "Missing target branch mapping for repo {} in workspace {}",
                        repo.id,
                        workspace_id
                    ))
                })?;
                Ok(match &store {
                    Some(store) => RepoWorkspaceInput::shared(
                        repo.clone(),
                        target_branch,
                        store.path_for(repo.id),
                    ),
                    None => RepoWorkspaceInput::new(repo.clone(), target_branch),
                })
            })
            .collect::<Result<_, ContainerError>>()?;

        Ok((repositories, workspace_inputs))
    }

    /// Best-effort repair of one cluster worktree that predates the shared
    /// store.
    ///
    /// Deliberately never fails the caller. Every outcome is either a no-op
    /// (the common case, once healed), a repair, or a refusal that leaves the
    /// worktree exactly as it was — and a workspace that cannot be healed is
    /// still better served by the ordinary `ensure` path reporting its own
    /// error than by this one masking it.
    async fn heal_cluster_worktree(
        &self,
        store: &SharedRepositoryStore,
        workspace_dir: &Path,
        input: &RepoWorkspaceInput,
        branch: &str,
    ) {
        let worktree_path = workspace_dir.join(&input.repo.name);
        // The store must hold the workspace's branch before anything is
        // re-pointed at it, or adoption would refuse — correctly, but for a
        // reason we can fix here by fetching from the checkout that still has
        // it.
        if let Err(e) = store.ensure(&input.repo, branch).await {
            tracing::warn!(
                repo = %input.repo.name,
                "could not prepare the shared store while healing {}: {e}",
                worktree_path.display()
            );
            return;
        }
        match store.adopt(&input.repo, &worktree_path, branch).await {
            Ok(AdoptOutcome::AlreadyPortable) => {}
            Ok(AdoptOutcome::Adopted { common_dir }) => tracing::info!(
                repo = %input.repo.name,
                "re-linked {} to {}",
                worktree_path.display(),
                common_dir.display()
            ),
            Ok(AdoptOutcome::Skipped { reason }) => tracing::info!(
                repo = %input.repo.name,
                "left {} alone: {reason}",
                worktree_path.display()
            ),
            Err(e) => tracing::warn!(
                repo = %input.repo.name,
                "could not re-link {}: {e}",
                worktree_path.display()
            ),
        }
    }

    /// The shared repository store this workspace's worktrees belong to, or
    /// `None` when they belong to the operator's registered checkout.
    ///
    /// `None` for every workspace when clustering is disabled, and for a
    /// `Local` placement when it is enabled — `Local` is a valid terminal state
    /// meaning "runs on the coordinator", which is what every workspace created
    /// before clustering has. Every other placement state, `Cleaning` included,
    /// belongs to the store: a clustered workspace's registrations live there,
    /// so cleaning it up against the registered checkout would delete the
    /// directory while unregistering nothing.
    async fn shared_repository_store_for(
        &self,
        workspace_id: Uuid,
    ) -> Result<Option<SharedRepositoryStore>, ContainerError> {
        if !self.cluster_config.enabled {
            return Ok(None);
        }
        let Some(placement) = WorkspacePlacement::find(&self.db.pool, workspace_id).await? else {
            return Ok(None);
        };
        // Exhaustive on purpose: a new placement state must be a compile error
        // here, not a silent fallthrough to the registered checkout.
        let uses_shared_store = match placement.placement_state {
            WorkspacePlacementState::Local => false,
            WorkspacePlacementState::Reserved
            | WorkspacePlacementState::Provisioning
            | WorkspacePlacementState::Ready
            | WorkspacePlacementState::Failed
            | WorkspacePlacementState::Cleaning => true,
        };
        if !uses_shared_store {
            return Ok(None);
        }
        let store = SharedRepositoryStore::new(
            &self.cluster_config.shared_root,
            self.repository_admin_locks.clone(),
        )
        .map_err(Self::map_workspace_manager_error)?;
        Ok(Some(store))
    }

    async fn get_child_from_store(&self, id: &Uuid) -> Option<Arc<RwLock<AsyncGroupChild>>> {
        let map = self.child_store.read().await;
        map.get(id).cloned()
    }

    async fn add_child_to_store(&self, id: Uuid, exec: AsyncGroupChild) {
        let mut map = self.child_store.write().await;
        map.insert(id, Arc::new(RwLock::new(exec)));
    }

    async fn remove_child_from_store(&self, id: &Uuid) {
        let mut map = self.child_store.write().await;
        map.remove(id);
    }

    // ===== Warm app-server registry (Phase 2) =========================
    // The registry is the single owner of a warm process's lifetime. It is
    // populated when a persistent app-server finishes a turn cleanly (and the
    // gate is on) and drained by the reap paths below. See
    // `specs/vk/826e-coding-agent-war/`.

    /// Whether warm coding-agent process reuse is enabled (env gate, default off,
    /// spec FR-8). Thin wrapper over `keep_warm_env_enabled` (unit-tested).
    fn warm_agents_enabled(&self) -> bool {
        keep_warm_env_enabled()
    }

    /// Move a warm child (already removed from `child_store`) into the registry,
    /// keyed by session id (spec FR-1/FR-7).
    /// Park a cleanly-finished warm child (still in `child_store`) into the warm
    /// registry, keyed by session. The child is inserted into `warm_app_servers`
    /// **before** it is removed from `child_store`, so a concurrent stop/teardown
    /// always finds it in at least one owner — never in a gap where it is invisible
    /// to both and could leak (Codex-review finding). Reaps any displaced
    /// same-session entry (one-per-session, FR-1/FR-7). A no-op if the child was
    /// already removed (e.g. a concurrent stop won the race).
    async fn park_warm_child(&self, session_id: Uuid, exec_id: Uuid, reuse: WarmReuseHandle) {
        let Some(child) = self.child_store.read().await.get(&exec_id).cloned() else {
            return;
        };
        register_warm_entry(&self.warm_app_servers, session_id, child, reuse).await;
        // The registry now owns the child; drop the child_store handle last.
        self.child_store.write().await.remove(&exec_id);
    }

    /// Reap the warm app-server for a session: kill its group + drop the entry.
    /// Idempotent. Closes the `Completed`-row teardown leak (spec FR-4).
    async fn reap_warm_server(&self, session_id: &Uuid) {
        reap_warm_entry(&self.warm_app_servers, session_id).await;
    }

    /// Take a live warm entry's child + reuse handle for a follow-up turn
    /// (spec FR-2/FR-6). Returns `None` on a miss / dead entry (cold start).
    async fn take_live_warm_server(
        &self,
        session_id: &Uuid,
    ) -> Option<(AsyncGroupChild, WarmReuseHandle)> {
        take_live_warm_entry(&self.warm_app_servers, session_id).await
    }

    /// Reap warm entries that are dead or idle past the timeout (spec FR-5/FR-6).
    async fn sweep_idle_warm_servers(&self) {
        sweep_idle_warm_entries(&self.warm_app_servers).await;
    }

    /// Reap every warm app-server on shutdown (spec FR-4c).
    async fn reap_all_warm_servers(&self) {
        reap_all_warm_entries(&self.warm_app_servers).await;
    }

    async fn add_cancellation_token(&self, id: Uuid, token: CancellationToken) {
        let mut map = self.cancellation_tokens.write().await;
        map.insert(id, token);
    }

    async fn take_cancellation_token(&self, id: &Uuid) -> Option<CancellationToken> {
        let mut map = self.cancellation_tokens.write().await;
        map.remove(id)
    }

    async fn add_db_stream_handle(&self, id: Uuid, handle: JoinHandle<()>) {
        let mut map = self.db_stream_handles.write().await;
        map.insert(id, handle);
    }

    async fn take_db_stream_handle(&self, id: &Uuid) -> Option<JoinHandle<()>> {
        let mut map = self.db_stream_handles.write().await;
        map.remove(id)
    }

    async fn add_exit_monitor_handle(&self, id: Uuid, handle: JoinHandle<()>) {
        let mut map = self.exit_monitor_handles.write().await;
        map.insert(id, handle);
    }

    async fn take_adopted_pgid(&self, id: &Uuid) -> Option<i32> {
        let mut map = self.adopted_pgids.write().await;
        map.remove(id)
    }

    /// Give the raw-log tailer a moment to catch up on the file's final
    /// bytes, then stop it. Call after the process has exited or been killed.
    async fn finish_raw_log_tailer(&self, id: &Uuid) {
        let handle = {
            let mut map = self.raw_log_tailers.write().await;
            map.remove(id)
        };
        if let Some(handle) = handle {
            tokio::time::sleep(Duration::from_millis(500)).await;
            handle.abort();
        }
    }

    /// Remove a just-finished execution's in-memory store and push its
    /// `Finished` sentinel — the shared tail every completion path in this
    /// file runs. Caches the store's settled entries to disk first (VAS-440):
    /// every call site here means the execution just left `Running`, so this
    /// is the one place guaranteed to hold the complete history without
    /// requiring a reader to ask for it before the store is gone.
    async fn finish_msg_store(&self, id: &Uuid) {
        let Some(store) = self.msg_stores.write().await.remove(id) else {
            return;
        };
        self.cache_finished_execution(id, &store).await;
        store.push_finished();
    }

    /// Leave a detached process running across a server shutdown so the next
    /// boot can adopt it. Forgets the child handle (defusing kill_on_drop)
    /// and stops its monitor tasks without touching the DB row, which stays
    /// Running.
    #[cfg(unix)]
    async fn detach_execution_for_handoff(&self, process: &ExecutionProcess) {
        if let Some(child_arc) = self.child_store.write().await.remove(&process.id) {
            // Leak one Arc clone so the child struct is never dropped and
            // kill_on_drop can't fire while the server shuts down.
            std::mem::forget(child_arc);
        }
        if let Some(handle) = self.take_exit_monitor_handle(&process.id).await {
            handle.abort();
        }
        if let Some(handle) = self.raw_log_tailers.write().await.remove(&process.id) {
            handle.abort();
        }
        self.take_adopted_pgid(&process.id).await;
        tracing::info!(
            "Leaving {:?} process {} running across restart",
            process.run_reason,
            process.id
        );
    }

    /// Watch an adopted process group and finalize its execution record when
    /// every process in the group has exited.
    #[cfg(unix)]
    fn spawn_adopted_exit_watcher(&self, exec_id: Uuid, pgid: i32) -> JoinHandle<()> {
        let container = self.clone();
        tokio::spawn(async move {
            while utils::process::process_group_alive(pgid) {
                tokio::time::sleep(Duration::from_secs(3)).await;
            }

            // The process ended on its own; the real exit code is unknowable
            // because the original parent is gone.
            if !ExecutionProcess::was_stopped(&container.db.pool, exec_id).await
                && let Err(e) = ExecutionProcess::update_completion(
                    &container.db.pool,
                    exec_id,
                    ExecutionProcessStatus::Failed,
                    None,
                )
                .await
            {
                tracing::error!(
                    "Failed to update completion for adopted execution {}: {}",
                    exec_id,
                    e
                );
            }

            container.take_adopted_pgid(&exec_id).await;
            container.finish_raw_log_tailer(&exec_id).await;
            container.finish_msg_store(&exec_id).await;
        })
    }

    /// Stop a process that was adopted from a previous server instance:
    /// there is no child handle, so the process group is signalled directly.
    async fn stop_adopted_execution(
        &self,
        execution_process: &ExecutionProcess,
        status: ExecutionProcessStatus,
        pgid: i32,
    ) -> Result<(), ContainerError> {
        let exit_code = if status == ExecutionProcessStatus::Completed {
            Some(0)
        } else {
            None
        };
        ExecutionProcess::update_completion(&self.db.pool, execution_process.id, status, exit_code)
            .await?;

        #[cfg(unix)]
        utils::process::kill_process_group_by_pgid(pgid).await;
        #[cfg(not(unix))]
        let _ = pgid;

        // The adopted liveness watcher would perform the same cleanup on its
        // next poll; abort it and clean up deterministically here instead.
        if let Some(handle) = self.take_exit_monitor_handle(&execution_process.id).await {
            handle.abort();
        }
        self.finish_raw_log_tailer(&execution_process.id).await;
        self.finish_msg_store(&execution_process.id).await;

        self.update_after_head_commits(execution_process.id).await;

        tracing::debug!(
            "Adopted execution process {} stopped successfully",
            execution_process.id
        );
        Ok(())
    }

    async fn take_exit_monitor_handle(&self, id: &Uuid) -> Option<JoinHandle<()>> {
        let mut map = self.exit_monitor_handles.write().await;
        map.remove(id)
    }

    async fn cleanup_workspace(&self, workspace: &Workspace) {
        let Some(container_ref) = &workspace.container_ref else {
            return;
        };
        let workspace_dir = PathBuf::from(container_ref);

        // Resolve through the same seam creation used, so a clustered
        // workspace's registrations are removed from the store that holds them.
        let workspace_inputs = match self.workspace_repo_inputs(workspace.id).await {
            Ok((_, inputs)) => inputs,
            Err(e) => {
                tracing::warn!(
                    "Could not resolve repositories for workspace {}: {}",
                    workspace.id,
                    e
                );
                Vec::new()
            }
        };

        let cleanup_succeeded = if workspace_inputs.is_empty() {
            tracing::warn!(
                "No repositories found for workspace {}, cleaning up workspace directory only",
                workspace.id
            );
            if workspace_dir.exists() {
                match tokio::fs::remove_dir_all(&workspace_dir).await {
                    Ok(()) => true,
                    Err(error) => {
                        tracing::warn!("Failed to remove workspace directory: {}", error);
                        false
                    }
                }
            } else {
                true
            }
        } else {
            match WorkspaceManager::cleanup_workspace(&workspace_dir, &workspace_inputs).await {
                Ok(()) => true,
                Err(error) => {
                    tracing::warn!(
                        "Failed to clean up workspace for workspace {}: {}",
                        workspace.id,
                        error
                    );
                    false
                }
            }
        };

        if !cleanup_succeeded {
            if self.cluster_config.enabled
                && let Err(error) =
                    WorkspacePlacement::cancel_cleanup(&self.db.pool, workspace.id).await
            {
                tracing::warn!(
                    workspace_id = %workspace.id,
                    %error,
                    "Failed to release unsuccessful clustered workspace cleanup claim"
                );
            }
            return;
        }

        if let Err(error) = Workspace::mark_worktree_deleted(&self.db.pool, workspace.id).await {
            tracing::warn!(
                workspace_id = %workspace.id,
                %error,
                "Failed to mark cleaned workspace worktree deleted"
            );
            if self.cluster_config.enabled
                && let Err(cancel_error) =
                    WorkspacePlacement::cancel_cleanup(&self.db.pool, workspace.id).await
            {
                tracing::warn!(
                    workspace_id = %workspace.id,
                    error = %cancel_error,
                    "Failed to release cleanup claim after deletion-state persistence failed"
                );
            }
            return;
        }

        if self.cluster_config.enabled {
            match WorkspacePlacement::finish_cleanup(&self.db.pool, workspace.id).await {
                Ok(true) => {}
                Ok(false) => tracing::warn!(
                    workspace_id = %workspace.id,
                    "Cleaned clustered workspace did not hold the cleanup fence"
                ),
                Err(error) => tracing::warn!(
                    workspace_id = %workspace.id,
                    %error,
                    "Failed to release clustered workspace cleanup fence"
                ),
            }
        }
    }

    async fn cleanup_expired_workspaces(&self) -> Result<(), DeploymentError> {
        if std::env::var("DISABLE_WORKTREE_CLEANUP").is_ok() {
            tracing::info!(
                "Expired workspace cleanup is disabled via DISABLE_WORKTREE_CLEANUP environment variable"
            );
            return Ok(());
        }

        let expired_workspaces = Workspace::find_expired_for_cleanup(&self.db.pool).await?;
        if expired_workspaces.is_empty() {
            tracing::debug!("No expired workspaces found");
            return Ok(());
        }
        tracing::info!(
            "Found {} expired workspaces to clean up",
            expired_workspaces.len()
        );
        for workspace in &expired_workspaces {
            if !self.cluster_workspace_cleanup_safe(workspace).await? {
                tracing::info!(
                    workspace_id = %workspace.id,
                    "Retaining expired clustered workspace because worker ownership is active or uncertain"
                );
                continue;
            }
            // Never auto-delete a workspace whose worktree still holds pending
            // work. Archival accelerates expiry to 1h (vs 72h), so an archived
            // workspace with uncommitted or untracked changes could otherwise be
            // reclaimed out from under unsaved work. Clean workspaces GC normally.
            match self.is_container_clean(workspace).await {
                Ok(true) => {}
                Ok(false) => {
                    tracing::info!(
                        "Retaining expired workspace {} from cleanup: worktree has uncommitted or untracked changes",
                        workspace.id
                    );
                    continue;
                }
                Err(e) => {
                    // If cleanliness can't be determined, keep the workspace
                    // rather than risk destroying unsaved work.
                    tracing::warn!(
                        "Skipping cleanup of expired workspace {}: could not determine worktree cleanliness: {}",
                        workspace.id,
                        e
                    );
                    continue;
                }
            }
            if self.cluster_config.enabled {
                let placement = WorkspacePlacement::find(&self.db.pool, workspace.id).await?;
                if placement
                    .as_ref()
                    .and_then(|value| value.worker_node_id)
                    .is_some()
                    && !WorkspacePlacement::begin_cleanup(&self.db.pool, workspace.id, Utc::now())
                        .await?
                {
                    tracing::info!(
                        workspace_id = %workspace.id,
                        "Retaining clustered workspace because cleanup ownership changed before reclamation"
                    );
                    continue;
                }
            }
            self.cleanup_workspace(workspace).await;
        }
        Ok(())
    }

    async fn cluster_workspace_cleanup_safe(
        &self,
        workspace: &Workspace,
    ) -> Result<bool, DeploymentError> {
        if !self.cluster_config.enabled {
            return Ok(true);
        }
        let Some(placement) = WorkspacePlacement::find(&self.db.pool, workspace.id).await? else {
            return Ok(false);
        };
        let Some(worker_node_id) = placement.worker_node_id else {
            return Ok(placement.placement_state == WorkspacePlacementState::Local);
        };
        let Some(worker) = WorkerNode::find_by_id(&self.db.pool, worker_node_id).await? else {
            return Ok(false);
        };
        let has_unsafe_jobs =
            ExecutionWorkerJob::has_unsafe_for_workspace(&self.db.pool, workspace.id).await?;
        Ok(worker_cleanup_evidence_safe(
            &worker,
            has_unsafe_jobs,
            Utc::now(),
        ))
    }

    fn spawn_workspace_cleanup(&self) {
        let container = self.clone();
        tokio::spawn(async move {
            container
                .workspace_manager
                .cleanup_orphan_workspaces(!container.cluster_config.enabled)
                .await;

            let mut cleanup_interval =
                tokio::time::interval(tokio::time::Duration::from_secs(1800)); // 30 minutes
            loop {
                cleanup_interval.tick().await;
                tracing::info!("Starting periodic workspace cleanup...");
                container
                    .cleanup_expired_workspaces()
                    .await
                    .unwrap_or_else(|e| {
                        tracing::error!("Failed to clean up expired workspaces: {}", e)
                    });
            }
        });

        // Warm app-server idle reaper (Phase 2). A separate, lighter cadence than
        // the 30-min workspace sweep so an idle warm process is reaped within a
        // few minutes of crossing WARM_IDLE_TIMEOUT rather than up to an extra
        // 30 minutes later. Level-triggered (Constitution VI); a no-op while the
        // registry is empty (i.e. whenever the keep-warm gate is off).
        let warm_container = self.clone();
        tokio::spawn(async move {
            let mut sweep_interval = tokio::time::interval(tokio::time::Duration::from_secs(300)); // 5 minutes
            loop {
                sweep_interval.tick().await;
                warm_container.sweep_idle_warm_servers().await;
            }
        });
    }

    /// Record the current HEAD commit for each repository as the "after" state.
    /// Errors are silently ignored since this runs after the main execution completes
    /// and failure should not block process finalization.
    async fn update_after_head_commits(&self, exec_id: Uuid) {
        if let Ok(ctx) = ExecutionProcess::load_context(&self.db.pool, exec_id).await {
            let workspace_root = self.workspace_to_current_dir(&ctx.workspace);
            for repo in &ctx.repos {
                let repo_path = workspace_root.join(&repo.name);
                if let Ok(head) = self.git().get_head_info(&repo_path) {
                    let _ = ExecutionProcessRepoState::update_after_head_commit(
                        &self.db.pool,
                        exec_id,
                        repo.id,
                        &head.oid,
                    )
                    .await;
                }
            }
        }
    }

    /// Get the commit message based on the execution run reason.
    async fn get_commit_message(&self, ctx: &ExecutionContext) -> String {
        match ctx.execution_process.run_reason {
            ExecutionProcessRunReason::CodingAgent => {
                // Try to retrieve the task summary from the coding agent turn
                // otherwise fallback to default message
                match CodingAgentTurn::find_by_execution_process_id(
                    &self.db().pool,
                    ctx.execution_process.id,
                )
                .await
                {
                    Ok(Some(turn)) if turn.summary.is_some() => turn.summary.unwrap(),
                    Ok(_) => {
                        tracing::debug!(
                            "No summary found for execution process {}, using default message",
                            ctx.execution_process.id
                        );
                        format!(
                            "Commit changes from coding agent for workspace {}",
                            ctx.workspace.id
                        )
                    }
                    Err(e) => {
                        tracing::debug!(
                            "Failed to retrieve summary for execution process {}: {}",
                            ctx.execution_process.id,
                            e
                        );
                        format!(
                            "Commit changes from coding agent for workspace {}",
                            ctx.workspace.id
                        )
                    }
                }
            }
            ExecutionProcessRunReason::CleanupScript => {
                format!("Cleanup script changes for workspace {}", ctx.workspace.id)
            }
            _ => format!(
                "Changes from execution process {}",
                ctx.execution_process.id
            ),
        }
    }

    /// Check which repos have uncommitted changes. Fails if any repo is inaccessible.
    fn check_repos_for_changes(
        &self,
        workspace_root: &Path,
        repos: &[Repo],
    ) -> Result<Vec<(Repo, PathBuf)>, ContainerError> {
        let git = GitService::new();
        let mut repos_with_changes = Vec::new();

        for repo in repos {
            let worktree_path = workspace_root.join(&repo.name);

            match git.get_worktree_status(&worktree_path) {
                Ok(ws) if !ws.entries.is_empty() => {
                    repos_with_changes.push((repo.clone(), worktree_path));
                }
                Ok(_) => {
                    tracing::debug!("No changes in repo '{}'", repo.name);
                }
                Err(e) => {
                    return Err(ContainerError::Other(anyhow!(
                        "Pre-flight check failed for repo '{}': {}",
                        repo.name,
                        e
                    )));
                }
            }
        }

        Ok(repos_with_changes)
    }

    async fn has_commits_from_execution(
        &self,
        ctx: &ExecutionContext,
    ) -> Result<bool, ContainerError> {
        let workspace_root = self.workspace_to_current_dir(&ctx.workspace);

        let repo_states = ExecutionProcessRepoState::find_by_execution_process_id(
            &self.db.pool,
            ctx.execution_process.id,
        )
        .await?;

        for repo in &ctx.repos {
            let repo_path = workspace_root.join(&repo.name);
            let current_head = self.git().get_head_info(&repo_path).ok().map(|h| h.oid);

            let before_head = repo_states
                .iter()
                .find(|s| s.repo_id == repo.id)
                .and_then(|s| s.before_head_commit.clone());

            if current_head != before_head {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Commit changes to each repo. Logs failures but continues with other repos.
    fn commit_repos(&self, repos_with_changes: Vec<(Repo, PathBuf)>, message: &str) -> bool {
        let mut any_committed = false;

        for (repo, worktree_path) in repos_with_changes {
            tracing::debug!(
                "Committing changes for repo '{}' at {:?}",
                repo.name,
                &worktree_path
            );

            match self.git().commit(&worktree_path, message) {
                Ok(true) => {
                    any_committed = true;
                    tracing::info!("Committed changes in repo '{}'", repo.name);
                }
                Ok(false) => {
                    tracing::warn!("No changes committed in repo '{}' (unexpected)", repo.name);
                }
                Err(e) => {
                    tracing::warn!("Failed to commit in repo '{}': {}", repo.name, e);
                }
            }
        }

        any_committed
    }

    /// Spawn a background task that polls the child process for completion and
    /// cleans up the execution entry when it exits.
    fn spawn_exit_monitor(
        &self,
        exec_id: &Uuid,
        session_id: Uuid,
        exit_signal: Option<ExecutorExitSignal>,
        keep_warm: bool,
        warm_reuse: Option<WarmReuseSignal>,
    ) -> JoinHandle<()> {
        let exec_id = *exec_id;
        let child_store = self.child_store.clone();
        let msg_stores = self.msg_stores.clone();
        let db = self.db.clone();
        let config = self.config.clone();
        let container = self.clone();
        let analytics = self.analytics.clone();

        let mut process_exit_rx = self.spawn_os_exit_watcher(exec_id);

        tokio::spawn(async move {
            // For natural-exit executors the child lifetime is the turn
            // lifetime. App-server executors report turn completion through a
            // separate signal, so their still-live child does not prove the
            // protocol turn remains active.
            let live_child_is_turn_evidence = exit_signal.is_none();
            let mut exit_signal_future = exit_signal
                .map(|rx| rx.boxed()) // wait for result
                .unwrap_or_else(|| std::future::pending().boxed()); // no signal, stall forever

            let status_result: std::io::Result<std::process::ExitStatus>;
            let mut reconciliation_timed_out = false;
            let final_output_timeout = wait_for_unfinalized_output(
                msg_stores.clone(),
                Some(child_store.clone()),
                exec_id,
                live_child_is_turn_evidence,
            );
            tokio::pin!(final_output_timeout);
            // True when a persistent app-server finished a turn cleanly and is
            // being left alive for reuse; also guards the tail cleanup below so
            // the warm child is not killed or dropped after finalization.
            let mut kept_warm = false;

            // Wait for process to exit, or exit signal from executor
            tokio::select! {
                // Exit signal with result.
                // Some coding agent processes do not automatically exit after processing the user request; instead the executor
                // signals when processing has finished to gracefully kill the process.
                exit_result = &mut exit_signal_future => {
                    // Executor signaled turn completion. Persistent app-servers
                    // (keep_warm) that finish a turn *cleanly* are left running so
                    // the next turn reuses the warm process — a turn is a protocol
                    // event, not a process lifetime. Any non-success result, or a
                    // turn that was explicitly stopped, falls through to the normal
                    // process-group kill so nothing lingers.
                    let is_success = matches!(exit_result, Ok(ExecutorExitResult::Success));
                    let was_stopped = ExecutionProcess::was_stopped(&db.pool, exec_id).await;
                    kept_warm = should_keep_warm(keep_warm, is_success, was_stopped);
                    if kept_warm {
                        tracing::info!(
                            "Keeping app-server warm across turn for execution {}",
                            exec_id
                        );
                    } else if let Some(child_lock) = child_store.read().await.get(&exec_id).cloned() {
                        let mut child = child_lock.write().await ;
                        if let Err(err) = command::kill_process_group(&mut child).await {
                            tracing::error!("Failed to kill process group after exit signal: {} {}", exec_id, err);
                        }
                    }

                    // Map the exit result to appropriate exit status
                    status_result = match exit_result {
                        Ok(ExecutorExitResult::Success) => Ok(success_exit_status()),
                        Ok(ExecutorExitResult::Failure) => Ok(failure_exit_status()),
                        Err(error) => Err(std::io::Error::other(format!(
                            "executor completion channel closed without terminal evidence: {error}"
                        ))),
                    };
                }
                // Process exit
                exit_status_result = &mut process_exit_rx => {
                    status_result = exit_status_result.unwrap_or_else(|e| Err(std::io::Error::other(e)));
                }
                _ = &mut final_output_timeout => {
                    reconciliation_timed_out = true;
                    tracing::warn!(
                        %exec_id,
                        timeout_seconds = FINAL_OUTPUT_RECONCILIATION_TIMEOUT.as_secs(),
                        "Final assistant output was not followed by terminal evidence; reconciling execution"
                    );
                    if let Some(child_lock) = child_store.read().await.get(&exec_id).cloned() {
                        let mut child = child_lock.write().await;
                        if let Err(error) = command::kill_process_group(&mut child).await {
                            tracing::error!(%exec_id, %error, "Failed to reap execution after final-output reconciliation timeout");
                        }
                    }
                    status_result = Err(std::io::Error::other(
                        "final output reconciliation timed out without terminal evidence"
                    ));
                }
            }

            // Park a cleanly-finished warm app-server into the registry *now*,
            // before finalization runs `try_start_next_action` / queued
            // follow-ups — so an immediately chained follow-up reuses the warm
            // server instead of cold-starting. The reuse handle was sent by the
            // executor before the turn ran, so `try_recv` gets it without an
            // `.await` (no window where the child is invisible to teardown). If
            // it is somehow absent we reap rather than strand the process.
            if kept_warm {
                let handle = warm_reuse.and_then(|mut rx| rx.try_recv().ok());
                match handle {
                    Some(handle) => {
                        container.park_warm_child(session_id, exec_id, handle).await;
                    }
                    None => {
                        if let Some(child_lock) = child_store.write().await.remove(&exec_id) {
                            let mut c = child_lock.write().await;
                            let _ = command::kill_process_group(&mut c).await;
                        }
                        tracing::warn!(
                            "Warm turn for session {session_id} produced no reuse handle; reaped"
                        );
                    }
                }
            }

            let (exit_code, status) = match status_result {
                Ok(exit_status) => {
                    let code = exit_status.code().unwrap_or(-1) as i64;
                    let status = if exit_status.success() {
                        ExecutionProcessStatus::Completed
                    } else {
                        ExecutionProcessStatus::Failed
                    };
                    (Some(code), status)
                }
                Err(_) if reconciliation_timed_out => (None, ExecutionProcessStatus::Indeterminate),
                Err(_) => (None, ExecutionProcessStatus::Failed),
            };

            if !ExecutionProcess::was_stopped(&db.pool, exec_id).await
                && let Err(e) =
                    update_completion_with_retry(&db, exec_id, status.clone(), exit_code).await
            {
                tracing::error!(%exec_id, ?status, "Failed to update execution process completion after bounded retries: {e}");
            }

            if let Ok(ctx) = ExecutionProcess::load_context(&db.pool, exec_id).await {
                // Update executor session summary if available
                if let Err(e) = container.update_executor_session_summary(&exec_id).await {
                    tracing::warn!("Failed to update executor session summary: {}", e);
                }

                let success = matches!(
                    ctx.execution_process.status,
                    ExecutionProcessStatus::Completed
                ) && exit_code == Some(0);

                let cleanup_done = matches!(
                    ctx.execution_process.run_reason,
                    ExecutionProcessRunReason::CleanupScript
                ) && !matches!(
                    ctx.execution_process.status,
                    ExecutionProcessStatus::Running
                );

                let mut already_finalized = false;

                if success || cleanup_done {
                    // Commit changes (if any) and get feedback about whether changes were made
                    let changes_committed = match container.try_commit_changes(&ctx).await {
                        Ok(committed) => committed,
                        Err(e) => {
                            tracing::error!("Failed to commit changes after execution: {}", e);
                            // Treat commit failures as if changes were made to be safe
                            true
                        }
                    };

                    let should_start_next = if matches!(
                        ctx.execution_process.run_reason,
                        ExecutionProcessRunReason::CodingAgent
                    ) {
                        // Check if agent made commits OR if we just committed uncommitted changes
                        changes_committed
                            || container
                                .has_commits_from_execution(&ctx)
                                .await
                                .unwrap_or(false)
                    } else {
                        true
                    };

                    if should_start_next {
                        // If the process exited successfully, start the next action
                        if let Err(e) = container.try_start_next_action(&ctx).await {
                            tracing::error!("Failed to start next action after completion: {}", e);
                        }
                    } else {
                        tracing::info!(
                            "Skipping cleanup script for workspace {} - no changes made by coding agent",
                            ctx.workspace.id
                        );

                        // The cleanup action is being bypassed, so it cannot reach
                        // the normal finalization block below. Consume a queued
                        // follow-up here before finalizing; otherwise the message
                        // remains in memory forever while the UI promises it will
                        // run when this execution finishes.
                        let started_queued_follow_up = match skipped_cleanup_action(
                            container.queued_message_service.has_queued(ctx.session.id),
                        ) {
                            SkippedCleanupAction::StartQueuedFollowUp => {
                                container
                                    .queued_message_service
                                    .wait_for_restart_resolution(ctx.session.id)
                                    .await;
                                match container.queued_message_service.take_queued(ctx.session.id) {
                                    Some(queued_msg) => {
                                        container
                                            .start_queued_follow_up_message(&ctx, &queued_msg)
                                            .await
                                    }
                                    // Cancellation can win between the status
                                    // check and the take; finalization is then
                                    // the correct fallback.
                                    None => false,
                                }
                            }
                            SkippedCleanupAction::Finalize => false,
                        };

                        if !started_queued_follow_up {
                            // Manually finalize since we're bypassing the cleanup
                            // action and did not replace it with a follow-up.
                            container.finalize_task(&ctx).await;
                        }
                        already_finalized = true;
                    }
                }

                if !already_finalized && container.should_finalize(&ctx) {
                    let has_chained_follow_up = ctx
                        .execution_process
                        .executor_action()
                        .ok()
                        .and_then(|action| action.next_action())
                        .is_some();
                    container
                        .queued_message_service
                        .wait_for_restart_resolution(ctx.session.id)
                        .await;
                    let mut started_queued_follow_up = false;

                    // Only execute queued messages if the execution succeeded
                    // If it failed, was killed or interrupted, just clear the queue and finalize
                    if let Some(queued_msg) =
                        container.queued_message_service.take_queued(ctx.session.id)
                    {
                        let should_execute_queued = (queued_msg.restart_agent
                            && ctx.execution_process.status == ExecutionProcessStatus::Failed)
                            || !matches!(
                                ctx.execution_process.status,
                                ExecutionProcessStatus::Failed
                                    | ExecutionProcessStatus::Killed
                                    | ExecutionProcessStatus::Interrupted
                            );
                        if should_execute_queued {
                            tracing::info!(
                                "Found queued message for session {}, starting follow-up execution",
                                ctx.session.id
                            );

                            if container
                                .start_queued_follow_up_message(&ctx, &queued_msg)
                                .await
                            {
                                started_queued_follow_up = true;
                            } else {
                                container.finalize_task(&ctx).await;
                            }
                        } else {
                            // Execution failed or was killed - discard the queued message and finalize
                            tracing::info!(
                                "Discarding queued message for session {} due to execution status {:?}",
                                ctx.session.id,
                                ctx.execution_process.status
                            );
                            container.finalize_task(&ctx).await;
                        }
                    } else if !started_queued_follow_up {
                        container.finalize_task(&ctx).await;
                    }

                    let should_mark_turn_unseen = matches!(
                        ctx.execution_process.run_reason,
                        ExecutionProcessRunReason::CodingAgent
                    ) && !has_chained_follow_up
                        && !started_queued_follow_up;

                    if should_mark_turn_unseen
                        && let Err(e) = CodingAgentTurn::mark_unseen_by_execution_process_id(
                            &db.pool,
                            ctx.execution_process.id,
                        )
                        .await
                    {
                        tracing::warn!(
                            "Failed to mark coding agent turn unseen for execution {}: {}",
                            ctx.execution_process.id,
                            e
                        );
                    }
                }

                // When a parallel setup script finishes and no coding agent is running,
                // consume any queued message that was stuck waiting
                if matches!(
                    ctx.execution_process.run_reason,
                    ExecutionProcessRunReason::SetupScript
                ) && !container.should_finalize(&ctx)
                {
                    let has_running_agent = ExecutionProcess::has_running_coding_agent_for_session(
                        &db.pool,
                        ctx.session.id,
                    )
                    .await
                    .unwrap_or(true);

                    if !has_running_agent
                        && let Some(queued_msg) =
                            container.queued_message_service.take_queued(ctx.session.id)
                    {
                        tracing::info!(
                            "Parallel setup script finished with queued message for session {}, starting follow-up",
                            ctx.session.id
                        );

                        if !container
                            .start_queued_follow_up_message(&ctx, &queued_msg)
                            .await
                        {
                            tracing::error!(
                                "Failed to start queued follow-up from setup script completion"
                            );
                        }
                    }
                }

                // Fire analytics event when CodingAgent execution has finished
                if config.read().await.analytics_enabled
                    && matches!(
                        &ctx.execution_process.run_reason,
                        ExecutionProcessRunReason::CodingAgent
                    )
                    && let Some(analytics) = &analytics
                {
                    analytics.analytics_service.track_event(&analytics.user_id, "task_attempt_finished", Some(json!({
                        "workspace_id": ctx.workspace.id.to_string(),
                        "session_id": ctx.session.id.to_string(),
                        "execution_success": matches!(ctx.execution_process.status, ExecutionProcessStatus::Completed),
                        "exit_code": ctx.execution_process.exit_code,
                    })));
                }

                // Sync workspace to remote after CodingAgent execution
                if matches!(
                    &ctx.execution_process.run_reason,
                    ExecutionProcessRunReason::CodingAgent
                ) && let Some(client) = &container.remote_client
                {
                    let stats = diff_stream::compute_diff_stats(
                        &container.db.pool,
                        &container.git,
                        &ctx.workspace,
                    )
                    .await;
                    let workspace_name =
                        Workspace::find_by_id_with_status(&container.db.pool, ctx.workspace.id)
                            .await
                            .ok()
                            .flatten()
                            .and_then(|ws| ws.workspace.name);
                    let client = client.clone();
                    let workspace_id = ctx.workspace.id;
                    let archived = ctx.workspace.archived;
                    tokio::spawn(async move {
                        remote_sync::sync_workspace_to_remote(
                            &client,
                            workspace_id,
                            workspace_name.map(Some),
                            Some(archived),
                            stats.as_ref(),
                        )
                        .await;
                    });
                }
            }

            // Now that commit/next-action/finalization steps for this process are complete,
            // capture the HEAD OID as the definitive "after" state (best-effort).
            container.update_after_head_commits(exec_id).await;

            // Let any raw-log tailer flush the file's final output, then wait
            // for DB persistence to complete before cleaning up the MsgStore
            container.finish_raw_log_tailer(&exec_id).await;
            let db_stream_handle = container.take_db_stream_handle(&exec_id).await;
            container.finish_msg_store(&exec_id).await;
            if let Some(handle) = db_stream_handle {
                let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
            }

            // SIGKILL any orphaned children (e.g. MCP servers) still in the
            // process group. The executor itself is already done — either it
            // exited naturally or was killed in the exit-signal branch above.
            //
            // A warm app-server is exempt: it was already moved into the
            // session-keyed `warm_app_servers` registry above (before
            // finalization), which becomes its single reaper
            // (stop/teardown/idle/death/shutdown), so it is no longer in
            // `child_store` and must not be killed here. Only non-warm children
            // are reaped at this tail.
            if !kept_warm {
                if let Some(child_lock) = child_store.read().await.get(&exec_id).cloned() {
                    let mut child = child_lock.write().await;
                    let _ = child.start_kill();
                }
                child_store.write().await.remove(&exec_id);
            }
            let mut controls = container.mcp_refresh_controls.write().await;
            if controls
                .get(&session_id)
                .is_some_and(|(control_exec_id, _)| *control_exec_id == exec_id)
            {
                controls.remove(&session_id);
            }
        })
    }

    fn spawn_os_exit_watcher(
        &self,
        exec_id: Uuid,
    ) -> tokio::sync::oneshot::Receiver<std::io::Result<std::process::ExitStatus>> {
        let (tx, rx) = tokio::sync::oneshot::channel::<std::io::Result<std::process::ExitStatus>>();
        let child_store = self.child_store.clone();
        tokio::spawn(async move {
            loop {
                // The exit monitor dropping its receiver means no one is
                // listening anymore — e.g. after a clean warm turn the child
                // is deliberately left alive. Stop polling instead of spinning
                // against a long-lived warm process.
                if tx.is_closed() {
                    break;
                }
                let child_lock = {
                    let map = child_store.read().await;
                    map.get(&exec_id).cloned()
                };
                if let Some(child_lock) = child_lock {
                    let mut child_handler = child_lock.write().await;
                    match child_handler.try_wait() {
                        Ok(Some(status)) => {
                            let _ = tx.send(Ok(status));
                            break;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            let _ = tx.send(Err(e));
                            break;
                        }
                    }
                } else {
                    let _ = tx.send(Err(io::Error::other(format!(
                        "Child handle missing for {exec_id}"
                    ))));
                    break;
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        });
        rx
    }

    fn dir_name_from_workspace(workspace_id: &Uuid, task_title: &str) -> String {
        let task_title_id = git_branch_id(task_title);
        format!("{}-{}", short_uuid(workspace_id), task_title_id)
    }

    /// Stream a detached process's raw log file into a fresh MsgStore,
    /// following the file as the process appends to it. Replays from the
    /// start of the file so the store's history is complete even when the
    /// process was adopted from a previous server instance.
    async fn track_raw_file_msgs_in_store(&self, id: Uuid, path: PathBuf) {
        let store = Arc::new(MsgStore::new());
        self.msg_stores.write().await.insert(id, store.clone());

        let handle = tokio::spawn(async move {
            use tokio::io::AsyncReadExt;

            // The file is created at spawn (or verified during adoption), but
            // retry briefly in case creation races this task.
            let mut file = {
                let mut attempts = 0;
                loop {
                    match tokio::fs::File::open(&path).await {
                        Ok(f) => break f,
                        Err(e) if attempts < 20 => {
                            attempts += 1;
                            tracing::debug!(
                                "Raw log file {} not yet available ({}), retrying",
                                path.display(),
                                e
                            );
                            tokio::time::sleep(Duration::from_millis(250)).await;
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Giving up opening raw log file {} for execution {}: {}",
                                path.display(),
                                id,
                                e
                            );
                            return;
                        }
                    }
                }
            };

            let mut buf = vec![0u8; 64 * 1024];
            loop {
                match file.read(&mut buf).await {
                    // At EOF; wait for the process to append more
                    Ok(0) => tokio::time::sleep(Duration::from_millis(250)).await,
                    Ok(n) => store.push_stdout(String::from_utf8_lossy(&buf[..n]).into_owned()),
                    Err(e) => {
                        tracing::warn!("Raw log tailer for execution {} failed: {}", id, e);
                        break;
                    }
                }
            }
        });
        self.raw_log_tailers.write().await.insert(id, handle);
    }

    async fn track_child_msgs_in_store(&self, id: Uuid, child: &mut AsyncGroupChild) {
        let store = Arc::new(MsgStore::new());

        // stdout/stderr may be absent when re-tracking a warm-reused child: its
        // original streams were consumed by the first turn's forwarder (a warm
        // follow-up installs a fresh stdout pipe but has no new stderr). Treat a
        // missing stream as empty instead of panicking (Phase 2 reuse path).
        let out: futures::stream::BoxStream<'static, Result<LogMsg, io::Error>> =
            match child.inner().stdout.take() {
                Some(out) => ReaderStream::new(out)
                    .map_ok(|chunk| LogMsg::Stdout(String::from_utf8_lossy(&chunk).into_owned()))
                    .boxed(),
                None => futures::stream::empty().boxed(),
            };
        let err: futures::stream::BoxStream<'static, Result<LogMsg, io::Error>> =
            match child.inner().stderr.take() {
                Some(err) => ReaderStream::new(err)
                    .map_ok(|chunk| LogMsg::Stderr(String::from_utf8_lossy(&chunk).into_owned()))
                    .boxed(),
                None => futures::stream::empty().boxed(),
            };

        // If you have a JSON Patch source, map it to LogMsg::JsonPatch too, then select all three.

        // Merge and forward into the store
        let merged = select(out, err); // Stream<Item = Result<LogMsg, io::Error>>
        store.clone().spawn_forwarder(merged);

        let mut map = self.msg_stores().write().await;
        map.insert(id, store);
    }

    async fn track_worker_msgs_in_store(
        &self,
        execution_process: &ExecutionProcess,
        worker_node_id: Uuid,
    ) -> Result<(), ContainerError> {
        let client = self.worker_client.clone().ok_or_else(|| {
            ContainerError::Other(anyhow!("Cluster worker client is not configured"))
        })?;
        let coordinator_id = self.cluster_config.coordinator_id.ok_or_else(|| {
            ContainerError::Other(anyhow!("Cluster coordinator identity is missing"))
        })?;
        let execution_id = execution_process.id;
        let base_executor = match &execution_process.executor_action.0 {
            ExecutorActionField::ExecutorAction(action) => action.base_executor(),
            ExecutorActionField::Other(_) => None,
        };
        let live_worker_lease_is_turn_evidence = worker_lease_is_turn_evidence(base_executor);
        let store = Arc::new(MsgStore::new());
        self.msg_stores
            .write()
            .await
            .insert(execution_id, store.clone());
        let db = self.db.clone();
        let container = self.clone();
        let handle = tokio::spawn(async move {
            let mut cursor = 0_u64;
            let mut retry_delay = Duration::from_millis(100);
            let mut final_output_deadline: Option<tokio::time::Instant> = None;
            'poll: loop {
                let batch = match client.events(worker_node_id, execution_id, cursor).await {
                    Ok(batch) => {
                        retry_delay = Duration::from_millis(100);
                        batch
                    }
                    Err(services::services::cluster::WorkerClientError::ReplayGap { .. }) => {
                        let _ = ExecutionWorkerJob::mark_output_incomplete(&db.pool, execution_id)
                            .await;
                        let _ = ExecutionWorkerJob::update_state(
                            &db.pool,
                            execution_id,
                            ExecutionWorkerDispatchState::Indeterminate,
                            None,
                            Some(Utc::now()),
                        )
                        .await;
                        if let Err(error) = update_completion_with_retry(
                            &db,
                            execution_id,
                            ExecutionProcessStatus::Indeterminate,
                            None,
                        )
                        .await
                        {
                            tracing::error!(%execution_id, %error, "Failed to persist replay-gap reconciliation");
                            tokio::time::sleep(retry_delay).await;
                            continue;
                        }
                        store.push(LogMsg::Stderr(
                            "Worker output replay gap; execution state is indeterminate".into(),
                        ));
                        // Remove (and cache) before finishing so a client that
                        // subscribes after this point re-derives from disk
                        // instead of attaching to a store whose live broadcast
                        // will never carry another message (see the terminal
                        // arm below for the full explanation).
                        container.finish_msg_store(&execution_id).await;
                        break;
                    }
                    Err(error) => {
                        tracing::warn!(
                            execution_id = %execution_id,
                            worker_node_id = %worker_node_id,
                            "Worker event poll failed; retrying: {error}"
                        );
                        if final_output_deadline
                            .is_some_and(|deadline| tokio::time::Instant::now() >= deadline)
                        {
                            tracing::warn!(
                                %execution_id,
                                %worker_node_id,
                                "Worker became unreachable after final output; marking execution indeterminate"
                            );
                            if let Err(update_error) =
                                mark_remote_execution_indeterminate(&db, execution_id).await
                            {
                                tracing::error!(%execution_id, %update_error, "Failed to reconcile unreachable worker execution");
                                tokio::time::sleep(retry_delay).await;
                                continue 'poll;
                            }
                            container.finalize_remote_execution(execution_id).await;
                            container.finish_msg_store(&execution_id).await;
                            break;
                        }
                        tokio::time::sleep(retry_delay).await;
                        retry_delay = (retry_delay * 2).min(Duration::from_secs(5));
                        continue;
                    }
                };

                let mut terminal = None;
                let had_events = !batch.events.is_empty();
                for event in batch.events {
                    cursor = event.sequence;
                    match event.payload {
                        ExecutionEventPayload::Stdout { data_base64 } => {
                            push_worker_bytes(&store, &data_base64, false);
                        }
                        ExecutionEventPayload::Stderr { data_base64 } => {
                            push_worker_bytes(&store, &data_base64, true);
                        }
                        ExecutionEventPayload::Structured { json } => {
                            if let Ok(message) = serde_json::from_str::<LogMsg>(&json) {
                                if let Some(is_final) = normalized_final_assistant_state(&message) {
                                    final_output_deadline = is_final.then(|| {
                                        tokio::time::Instant::now()
                                            + FINAL_OUTPUT_RECONCILIATION_TIMEOUT
                                    });
                                }
                                store.push(message);
                            } else {
                                store.push_stdout(format!("{json}\n"));
                            }
                        }
                        ExecutionEventPayload::Completed(evidence) => {
                            terminal = Some((
                                ExecutionWorkerDispatchState::Completed,
                                ExecutionProcessStatus::Completed,
                                evidence,
                            ));
                        }
                        ExecutionEventPayload::Failed(evidence) => {
                            terminal = Some((
                                ExecutionWorkerDispatchState::Failed,
                                ExecutionProcessStatus::Failed,
                                evidence,
                            ));
                        }
                        ExecutionEventPayload::Killed(evidence) => {
                            terminal = Some((
                                ExecutionWorkerDispatchState::Killed,
                                ExecutionProcessStatus::Killed,
                                evidence,
                            ));
                        }
                        ExecutionEventPayload::Interrupted(evidence) => {
                            terminal = Some((
                                ExecutionWorkerDispatchState::Interrupted,
                                ExecutionProcessStatus::Interrupted,
                                evidence,
                            ));
                        }
                        ExecutionEventPayload::Indeterminate { reason } => {
                            store.push(LogMsg::Stderr(format!(
                                "Worker reported an indeterminate execution: {reason}"
                            )));
                            let _ = ExecutionWorkerJob::update_state(
                                &db.pool,
                                execution_id,
                                ExecutionWorkerDispatchState::Indeterminate,
                                None,
                                Some(Utc::now()),
                            )
                            .await;
                            loop {
                                match update_completion_with_retry(
                                    &db,
                                    execution_id,
                                    ExecutionProcessStatus::Indeterminate,
                                    None,
                                )
                                .await
                                {
                                    Ok(()) => break,
                                    Err(error) => {
                                        tracing::error!(%execution_id, %error, "Worker-reported indeterminate evidence remains pending");
                                        tokio::time::sleep(retry_delay).await;
                                        retry_delay = (retry_delay * 2).min(Duration::from_secs(5));
                                    }
                                }
                            }
                            container.finish_msg_store(&execution_id).await;
                            return;
                        }
                        ExecutionEventPayload::InteractionRequested(interaction) => {
                            final_output_deadline = None;
                            container.route_worker_interaction(
                                execution_id,
                                worker_node_id,
                                interaction,
                            );
                        }
                        ExecutionEventPayload::Accepted => {}
                        ExecutionEventPayload::Starting => {
                            final_output_deadline = None;
                        }
                        ExecutionEventPayload::InteractionAcknowledged { .. }
                        | ExecutionEventPayload::Preview(_) => {}
                    }
                }

                if had_events && final_output_deadline.is_some() {
                    final_output_deadline =
                        Some(tokio::time::Instant::now() + FINAL_OUTPUT_RECONCILIATION_TIMEOUT);
                }

                // Terminal events are deliberately not acknowledged until the
                // process-row transition below succeeds; the worker retains
                // replay authority if this coordinator dies while persisting.
                if should_ack_worker_batch(cursor, terminal.is_some()) {
                    let _ = ExecutionWorkerJob::acknowledge_sequence(
                        &db.pool,
                        execution_id,
                        cursor as i64,
                        batch.latest_available as i64,
                    )
                    .await;
                    let acknowledgement = EventAcknowledgement {
                        authority: RequestAuthority {
                            protocol_version: PROTOCOL_VERSION,
                            coordinator_id,
                            worker_node_id,
                            correlation_id: execution_id,
                            issued_at: Utc::now(),
                            nonce: Uuid::new_v4().to_string(),
                        },
                        execution_id,
                        highest_contiguous_sequence: cursor,
                    };
                    if let Err(error) = client.acknowledge(worker_node_id, &acknowledgement).await {
                        tracing::warn!(%execution_id, "Worker event acknowledgement failed: {error}");
                    }
                }

                if let Some((worker_state, process_state, evidence)) = terminal {
                    let evidence_json = serde_json::to_value(&evidence).ok();
                    let _ = ExecutionWorkerJob::update_state(
                        &db.pool,
                        execution_id,
                        worker_state,
                        evidence_json.as_ref(),
                        Some(evidence.observed_at),
                    )
                    .await;
                    let exit_code = evidence.exit_code.map(i64::from);
                    if !ExecutionProcess::was_stopped(&db.pool, execution_id).await {
                        // Do not acknowledge or discard terminal evidence until
                        // the authoritative process row contains it. Each retry
                        // burst is bounded; persistent database failure keeps
                        // this captured evidence resident instead of polling
                        // past and permanently losing it.
                        loop {
                            match update_completion_with_retry(
                                &db,
                                execution_id,
                                process_state.clone(),
                                exit_code,
                            )
                            .await
                            {
                                Ok(()) => break,
                                Err(error) => {
                                    tracing::error!(%execution_id, ?process_state, %error, "Terminal evidence remains pending; retrying before acknowledgement");
                                    tokio::time::sleep(retry_delay).await;
                                    retry_delay = (retry_delay * 2).min(Duration::from_secs(5));
                                }
                            }
                        }
                    }
                    // Persistence now owns the terminal truth, so release the
                    // worker's replay buffer through both acknowledgement
                    // channels before finalizing this tracker.
                    let _ = ExecutionWorkerJob::acknowledge_sequence(
                        &db.pool,
                        execution_id,
                        cursor as i64,
                        batch.latest_available as i64,
                    )
                    .await;
                    let acknowledgement = EventAcknowledgement {
                        authority: RequestAuthority {
                            protocol_version: PROTOCOL_VERSION,
                            coordinator_id,
                            worker_node_id,
                            correlation_id: execution_id,
                            issued_at: Utc::now(),
                            nonce: Uuid::new_v4().to_string(),
                        },
                        execution_id,
                        highest_contiguous_sequence: cursor,
                    };
                    if let Err(error) = client.acknowledge(worker_node_id, &acknowledgement).await {
                        tracing::warn!(%execution_id, "Terminal worker acknowledgement failed after persistence: {error}");
                    }
                    container.finalize_remote_execution(execution_id).await;
                    // Remove (and cache) from the map before pushing Finished.
                    // Historic replay (`stream_normalized_logs`) reads a
                    // resident store via `history_plus_stream()`, which chains
                    // the buffered history onto a *live* broadcast
                    // subscription. A late subscriber's history replay
                    // includes this store's past `Finished` entry, but the
                    // filter used by that replay path drops non-JsonPatch
                    // entries — so the sentinel is discarded and the replay
                    // falls through to the live half, which never yields
                    // again for a store nothing will ever push to. Removing
                    // the store here ensures any later subscriber instead
                    // falls back to the on-disk/materialized log path, which
                    // terminates correctly.
                    container.finish_msg_store(&execution_id).await;
                    break;
                }

                if final_output_deadline
                    .is_some_and(|deadline| tokio::time::Instant::now() >= deadline)
                {
                    match ExecutionWorkerJob::find_by_execution_id(&db.pool, execution_id).await {
                        Ok(Some(job))
                            if worker_job_has_positive_liveness(
                                &job,
                                Utc::now(),
                                live_worker_lease_is_turn_evidence,
                            ) =>
                        {
                            // A current job lease is authoritative positive
                            // liveness. Do not turn quiet final text into a
                            // cancellation while the worker still owns work.
                            final_output_deadline = Some(
                                tokio::time::Instant::now() + FINAL_OUTPUT_RECONCILIATION_TIMEOUT,
                            );
                            continue;
                        }
                        Err(error) => {
                            tracing::warn!(%execution_id, %error, "Unable to verify worker liveness; deferring destructive reconciliation");
                            final_output_deadline = Some(
                                tokio::time::Instant::now() + FINAL_OUTPUT_RECONCILIATION_TIMEOUT,
                            );
                            continue;
                        }
                        _ => {}
                    }
                    tracing::warn!(
                        %execution_id,
                        %worker_node_id,
                        "Worker final assistant output was not followed by terminal evidence; marking execution indeterminate"
                    );
                    let cancellation = CancellationRequest {
                        authority: RequestAuthority {
                            protocol_version: PROTOCOL_VERSION,
                            coordinator_id,
                            worker_node_id,
                            correlation_id: execution_id,
                            issued_at: Utc::now(),
                            nonce: Uuid::new_v4().to_string(),
                        },
                        execution_id,
                        graceful_timeout_seconds: 5,
                        terminate_timeout_seconds: 5,
                    };
                    if let Err(error) = client.cancel(worker_node_id, &cancellation).await {
                        tracing::warn!(%execution_id, %worker_node_id, %error, "Failed to cancel worker after final-output reconciliation timeout");
                    }
                    if let Err(error) = ExecutionWorkerJob::update_state(
                        &db.pool,
                        execution_id,
                        ExecutionWorkerDispatchState::Indeterminate,
                        None,
                        Some(Utc::now()),
                    )
                    .await
                    {
                        tracing::error!(%execution_id, %error, "Failed to persist indeterminate worker reconciliation state");
                    }
                    if let Err(error) = update_completion_with_retry(
                        &db,
                        execution_id,
                        ExecutionProcessStatus::Indeterminate,
                        None,
                    )
                    .await
                    {
                        tracing::error!(%execution_id, %error, "Failed to persist final-output reconciliation after bounded retries");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                    store.push(LogMsg::Stderr(
                        "Final response arrived without worker terminal evidence; execution state is indeterminate"
                            .into(),
                    ));
                    container.finalize_remote_execution(execution_id).await;
                    container.finish_msg_store(&execution_id).await;
                    break;
                }

                if batch.latest_available <= cursor {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        });
        self.add_exit_monitor_handle(execution_id, handle).await;
        Ok(())
    }

    async fn finalize_remote_execution(&self, execution_id: Uuid) {
        let Ok(ctx) = ExecutionProcess::load_context(&self.db.pool, execution_id).await else {
            tracing::warn!(%execution_id, "Could not load remote execution context for finalization");
            return;
        };
        if ctx.execution_process.status == ExecutionProcessStatus::Completed {
            if let Err(error) = self.try_commit_changes(&ctx).await {
                tracing::error!(%execution_id, "Remote execution commit failed: {error}");
            }
            if let Err(error) = self.try_start_next_action(&ctx).await {
                tracing::error!(%execution_id, "Remote execution next action failed: {error}");
            }
        }
        if self.should_finalize(&ctx) {
            self.finalize_task(&ctx).await;
        }
        self.update_after_head_commits(execution_id).await;
    }

    /// Create a live diff log stream for ongoing attempts for WebSocket
    /// Returns a stream that owns the filesystem watcher - when dropped, watcher is cleaned up
    async fn create_live_diff_stream(
        &self,
        args: diff_stream::DiffStreamArgs,
    ) -> Result<DiffStreamHandle, ContainerError> {
        diff_stream::create(args)
            .await
            .map_err(|e| ContainerError::Other(anyhow!("{e}")))
    }

    /// Extract the last assistant message from the MsgStore history
    fn extract_last_assistant_message(&self, exec_id: &Uuid) -> Option<String> {
        // Get the MsgStore for this execution
        let msg_stores = self.msg_stores.try_read().ok()?;
        let msg_store = msg_stores.get(exec_id)?;

        // Get the history and scan in reverse for the last assistant message
        let history = msg_store.get_history();

        for msg in history.iter().rev() {
            if let LogMsg::JsonPatch(patch) = msg {
                // Try to extract a NormalizedEntry from the patch
                if let Some((_, entry)) = extract_normalized_entry_from_patch(patch)
                    && matches!(entry.entry_type, NormalizedEntryType::AssistantMessage)
                {
                    let content = entry.content.trim();
                    if !content.is_empty() {
                        const MAX_SUMMARY_LENGTH: usize = 4096;
                        if content.len() > MAX_SUMMARY_LENGTH {
                            let truncated = truncate_to_char_boundary(content, MAX_SUMMARY_LENGTH);
                            return Some(format!("{truncated}..."));
                        }
                        return Some(content.to_string());
                    }
                }
            }
        }

        None
    }

    /// Update the coding agent turn summary with the final assistant message
    async fn update_executor_session_summary(&self, exec_id: &Uuid) -> Result<(), anyhow::Error> {
        // Check if there's a coding agent turn for this execution process
        let turn = CodingAgentTurn::find_by_execution_process_id(&self.db.pool, *exec_id).await?;

        if let Some(turn) = turn {
            // Only update if summary is not already set
            if turn.summary.is_none() {
                if let Some(summary) = self.extract_last_assistant_message(exec_id) {
                    CodingAgentTurn::update_summary(&self.db.pool, *exec_id, &summary).await?;
                } else {
                    tracing::debug!("No assistant message found for execution {}", exec_id);
                }
            }
        }

        Ok(())
    }

    /// Copy project files and workspace attachments to the workspace.
    /// Skips files that already exist (fast no-op if all exist).
    async fn copy_files_and_images(
        &self,
        workspace_dir: &Path,
        workspace: &Workspace,
    ) -> Result<(), ContainerError> {
        let repos = WorkspaceRepo::find_repos_with_copy_files(&self.db.pool, workspace.id).await?;

        for repo in &repos {
            if let Some(copy_files) = &repo.copy_files
                && !copy_files.trim().is_empty()
            {
                let worktree_path = workspace_dir.join(&repo.name);
                self.copy_project_files(&repo.path, &worktree_path, copy_files)
                    .await
                    .unwrap_or_else(|e| {
                        tracing::warn!(
                            "Failed to copy project files for repo '{}': {}",
                            repo.name,
                            e
                        );
                    });
            }
        }

        let agent_working_dir = Session::find_latest_by_workspace_id(&self.db.pool, workspace.id)
            .await?
            .and_then(|session| session.agent_working_dir);

        if let Err(e) = self
            .file_service
            .copy_files_by_workspace_to_worktree(
                workspace_dir,
                workspace.id,
                agent_working_dir.as_deref(),
            )
            .await
        {
            tracing::warn!("Failed to copy workspace files to workspace: {}", e);
        }

        Ok(())
    }

    /// Create workspace-level CLAUDE.md and AGENTS.md files that import from each repo.
    /// Uses the @import syntax to reference each repo's config files.
    /// Skips creating files if they already exist or if no repos have the source file.
    async fn create_workspace_config_files(
        workspace_dir: &Path,
        repos: &[Repo],
    ) -> Result<(), ContainerError> {
        const CONFIG_FILES: [&str; 2] = ["CLAUDE.md", "AGENTS.md"];

        for config_file in CONFIG_FILES {
            let workspace_config_path = workspace_dir.join(config_file);

            if workspace_config_path.exists() {
                tracing::trace!(
                    "Workspace config file {} already exists, skipping",
                    config_file
                );
                continue;
            }

            let mut import_lines = Vec::new();
            for repo in repos {
                let repo_config_path = workspace_dir.join(&repo.name).join(config_file);
                if repo_config_path.exists() {
                    import_lines.push(format!("@{}/{}", repo.name, config_file));
                }
            }

            if import_lines.is_empty() {
                tracing::trace!(
                    "No repos have {}, skipping workspace config creation",
                    config_file
                );
                continue;
            }

            let content = import_lines.join("\n") + "\n";
            if let Err(e) = tokio::fs::write(&workspace_config_path, &content).await {
                tracing::warn!(
                    "Failed to create workspace config file {}: {}",
                    config_file,
                    e
                );
                continue;
            }

            tracing::info!(
                "Created workspace {} with {} import(s)",
                config_file,
                import_lines.len()
            );
        }

        Ok(())
    }

    /// Start a follow-up execution from a queued message
    async fn start_queued_follow_up_message(
        &self,
        ctx: &ExecutionContext,
        queued_msg: &services::services::queued_message::QueuedMessage,
    ) -> bool {
        if queued_msg.restart_agent {
            self.reap_warm_server(&ctx.session.id).await;
        }
        if let Err(e) =
            Scratch::delete(&self.db.pool, ctx.session.id, &ScratchType::DraftFollowUp).await
        {
            tracing::warn!("Failed to delete scratch after consuming queued message: {e}");
        }

        if let Err(e) = self.start_queued_follow_up(ctx, &queued_msg.data).await {
            tracing::error!("Failed to start queued follow-up: {e}");
            false
        } else {
            true
        }
    }

    /// Start a follow-up execution from queued message data.
    async fn start_queued_follow_up(
        &self,
        ctx: &ExecutionContext,
        queued_data: &DraftFollowUpData,
    ) -> Result<ExecutionProcess, ContainerError> {
        let executor_profile_id = queued_data.executor_config.profile_id();

        // Validate executor matches session if session has prior executions
        let expected_executor: Option<String> =
            ExecutionProcess::latest_executor_profile_for_session(&self.db.pool, ctx.session.id)
                .await?
                .map(|profile| profile.executor.to_string())
                .or_else(|| ctx.session.executor.clone());

        if let Some(expected) = expected_executor {
            let actual = executor_profile_id.executor.to_string();
            if expected != actual {
                return Err(SessionError::ExecutorMismatch { expected, actual }.into());
            }
        }

        if ctx.session.executor.is_none() {
            Session::update_executor(
                &self.db.pool,
                ctx.session.id,
                &executor_profile_id.executor.to_string(),
            )
            .await?;
        }

        // Get latest agent turn for session continuity (from coding agent turns)
        let latest_session_info =
            CodingAgentTurn::find_latest_session_info(&self.db.pool, ctx.session.id).await?;

        let repos =
            WorkspaceRepo::find_repos_for_workspace(&self.db.pool, ctx.workspace.id).await?;
        let cleanup_action = self.cleanup_actions_for_repos(&repos);

        let working_dir = ctx
            .session
            .agent_working_dir
            .as_ref()
            .filter(|dir| !dir.is_empty())
            .cloned();

        let action_type = if let Some(info) = latest_session_info {
            ExecutorActionType::CodingAgentFollowUpRequest(CodingAgentFollowUpRequest {
                prompt: queued_data.message.clone(),
                session_id: info.session_id,
                reset_to_message_id: None,
                executor_config: queued_data.executor_config.clone(),
                working_dir: working_dir.clone(),
            })
        } else {
            ExecutorActionType::CodingAgentInitialRequest(CodingAgentInitialRequest {
                prompt: queued_data.message.clone(),
                executor_config: queued_data.executor_config.clone(),
                working_dir,
            })
        };

        let action = ExecutorAction::new(action_type, cleanup_action.map(Box::new));

        self.start_execution(
            &ctx.workspace,
            &ctx.session,
            &action,
            &ExecutionProcessRunReason::CodingAgent,
        )
        .await
    }

    /// Resolve organization-level env vars for a workspace by mapping it to
    /// its remote project and fetching the decrypted values from the remote
    /// server. The mapping is tried in order of cost: the `remote_project_id`
    /// persisted on the workspace row (set at creation for issue-linked
    /// workspaces and on link/unlink), the legacy workspace → task → local
    /// project chain, and finally the remote server's own workspace record
    /// (persisted back onto the row once found). Returns an empty map when the
    /// workspace isn't linked to a remote project, no remote client is
    /// configured, or the fetch fails — org env vars are best-effort and must
    /// never block a workspace from starting.
    async fn resolve_org_env_vars_inner(&self, workspace: &Workspace) -> HashMap<String, String> {
        let Some(remote_client) = self.remote_client.as_ref() else {
            return HashMap::new();
        };

        let mut remote_project_id =
            match Workspace::get_remote_project_id(&self.db.pool, workspace.id).await {
                Ok(id) => id,
                Err(e) => {
                    tracing::warn!(
                        ?e,
                        "Failed to load workspace remote project while resolving org env vars"
                    );
                    None
                }
            };

        if remote_project_id.is_none()
            && let Some(task_id) = workspace.task_id
        {
            remote_project_id = match Task::find_by_id(&self.db.pool, task_id).await {
                Ok(Some(task)) => match Project::find_by_id(&self.db.pool, task.project_id).await {
                    Ok(Some(project)) => project.remote_project_id,
                    Ok(None) => None,
                    Err(e) => {
                        tracing::warn!(?e, "Failed to load project while resolving org env vars");
                        None
                    }
                },
                Ok(None) => None,
                Err(e) => {
                    tracing::warn!(?e, "Failed to load task while resolving org env vars");
                    None
                }
            };
        }

        let fetch = async {
            let remote_project_id = match remote_project_id {
                Some(id) => id,
                None => {
                    // Workspace-anchored flows link workspaces to remote
                    // projects without a local task/project record; ask the
                    // remote for the mapping and persist it so later spawns
                    // skip this round-trip.
                    match remote_client.get_workspace_by_local_id(workspace.id).await {
                        Ok(remote_ws) => {
                            if let Err(e) = Workspace::set_remote_project_id(
                                &self.db.pool,
                                workspace.id,
                                Some(remote_ws.project_id),
                            )
                            .await
                            {
                                tracing::warn!(
                                    ?e,
                                    "Failed to persist remote project id on workspace"
                                );
                            }
                            remote_ws.project_id
                        }
                        // Not linked to a remote workspace — local-only.
                        Err(RemoteClientError::Http { status: 404, .. }) => return None,
                        Err(e) => {
                            tracing::warn!(
                                ?e,
                                "Failed to resolve remote workspace while resolving org env vars"
                            );
                            return None;
                        }
                    }
                }
            };

            match remote_client.get_project_env_vars(remote_project_id).await {
                Ok(resp) => Some(resp),
                Err(e) => {
                    tracing::warn!(?e, %remote_project_id, "Failed to fetch org env vars from remote");
                    None
                }
            }
        };

        // Cap the wait: this runs on the spawn path for every execution, and the
        // remote client itself retries with a 30s per-request timeout. A short
        // outer timeout keeps a degraded remote from stalling workspace starts.
        let resp = match tokio::time::timeout(ORG_ENV_FETCH_TIMEOUT, fetch).await {
            Ok(Some(resp)) => resp,
            Ok(None) => return HashMap::new(),
            Err(_) => {
                tracing::warn!("Timed out fetching org env vars from remote");
                return HashMap::new();
            }
        };

        resp.env_vars
            .into_iter()
            .filter_map(|v| {
                if is_reserved_env_name(&v.name) {
                    // Don't let an org config clobber the workspace contract or an
                    // executor's own runtime/auth wiring.
                    tracing::warn!(name = %v.name, "Ignoring org env var with reserved name");
                    None
                } else {
                    Some((v.name, v.value))
                }
            })
            .collect()
    }
}

/// Timeout for the best-effort org env var fetch on the execution spawn path.
const ORG_ENV_FETCH_TIMEOUT: Duration = Duration::from_secs(5);

/// Env var names that Vibe Kanban or the coding-agent executors manage
/// internally. Org-provided values for these are ignored so an organization
/// config cannot clobber the workspace contract (`VK_*`) or break an executor's
/// own auth/runtime wiring (e.g. opencode's server password). Names are
/// case-sensitive to match process env semantics on Unix.
const RESERVED_ENV_PREFIXES: &[&str] = &["VK_"];
const RESERVED_ENV_NAMES: &[&str] = &[
    "GOBIN",
    "GOCACHE",
    "GOENV",
    "GOMODCACHE",
    "GOPATH",
    "GOTOOLCHAIN",
    "PATH",
    "HOME",
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "OPENCODE_SERVER_PASSWORD",
];

fn workspace_go_environment(
    workspace_root: &Path,
    inherited_path: Option<&std::ffi::OsStr>,
) -> HashMap<String, String> {
    let root = workspace_root.join(".vibe-kanban/go");
    let gobin = root.join("bin");
    let mut environment = HashMap::from([
        ("GOBIN".into(), gobin.to_string_lossy().into_owned()),
        (
            "GOCACHE".into(),
            root.join("build").to_string_lossy().into_owned(),
        ),
        (
            "GOENV".into(),
            root.join("env").to_string_lossy().into_owned(),
        ),
        (
            "GOMODCACHE".into(),
            root.join("mod").to_string_lossy().into_owned(),
        ),
        (
            "GOPATH".into(),
            root.join("path").to_string_lossy().into_owned(),
        ),
        ("GOTOOLCHAIN".into(), "auto".into()),
    ]);
    if let Some(inherited_path) = inherited_path {
        let path = std::env::join_paths(
            std::iter::once(gobin).chain(std::env::split_paths(inherited_path)),
        )
        .unwrap_or_else(|_| inherited_path.to_os_string());
        environment.insert("PATH".into(), path.to_string_lossy().into_owned());
    }
    environment
}

fn is_reserved_env_name(name: &str) -> bool {
    RESERVED_ENV_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
        || RESERVED_ENV_NAMES.contains(&name)
}

/// Decide whether a persistent app-server should be left running ("warm") when
/// its executor signals turn completion, instead of having its process group
/// killed. A turn is a protocol event, not a process lifetime — but only a
/// *clean* turn end keeps the process: any failure result, a non-warm executor,
/// or a turn that was explicitly stopped is reaped as usual. Kept pure so the
/// decision matrix is unit-testable without a container or database.
fn should_keep_warm(keep_warm: bool, is_success: bool, was_stopped: bool) -> bool {
    keep_warm && is_success && !was_stopped
}

fn failure_exit_status() -> std::process::ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        ExitStatusExt::from_raw(256) // Exit code 1 (shifted by 8 bits)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt;
        ExitStatusExt::from_raw(1)
    }
}

#[async_trait]
impl ContainerService for LocalContainerService {
    fn msg_stores(&self) -> &Arc<RwLock<HashMap<Uuid, Arc<MsgStore>>>> {
        &self.msg_stores
    }

    /// Drain the warm registry for this session on teardown (the leak fix — the
    /// warm process's turn row is `Completed` and the `Running`-only stop loop
    /// would skip it). See `specs/vk/826e-coding-agent-war/` (spec FR-4b).
    async fn reap_warm_processes_for_session(&self, session_id: Uuid) {
        self.reap_warm_server(&session_id).await;
        self.mcp_refresh_controls.write().await.remove(&session_id);
        self.mcp_refresh_coordinator.remove(session_id).await;
    }

    fn db(&self) -> &DBService {
        &self.db
    }

    fn git(&self) -> &GitService {
        &self.git
    }

    fn notification_service(&self) -> &NotificationService {
        &self.notification_service
    }

    async fn refresh_mcp_tools(
        &self,
        workspace_id: Uuid,
        session_id: Uuid,
    ) -> Result<McpRefreshResult, ContainerError> {
        let session = Session::find_by_id(&self.db.pool, session_id)
            .await?
            .ok_or(SessionError::NotFound)?;
        if session.workspace_id != workspace_id {
            return Err(ContainerError::Other(anyhow!(
                "Session does not belong to workspace"
            )));
        }
        let profile =
            ExecutionProcess::latest_executor_profile_for_session(&self.db.pool, session_id)
                .await?;
        let supported = profile
            .as_ref()
            .is_some_and(|profile| profile.executor == BaseCodingAgent::Codex);
        let result = self
            .mcp_refresh_coordinator
            .request(session_id, supported)
            .await;
        if result.status != McpRefreshStatus::PendingNextTurn {
            return Ok(result);
        }

        // A clustered execution owns both its scoped config and live Codex
        // control on the assigned worker. Resolve settings again here; the
        // dispatch-time snapshot is intentionally not reused.
        let latest_execution =
            ExecutionProcess::find_by_session_id(&self.db.pool, session_id, false)
                .await?
                .into_iter()
                .rev()
                .find(|process| process.run_reason == ExecutionProcessRunReason::CodingAgent);
        if let Some(execution) = latest_execution
            && let Some(worker_job) =
                ExecutionWorkerJob::find_by_execution_id(&self.db.pool, execution.id).await?
        {
            let Some(profile_id) = profile else {
                return Ok(self
                    .mcp_refresh_coordinator
                    .fail(session_id, McpRefreshErrorCategory::Unsupported)
                    .await
                    .unwrap_or(result));
            };
            let Some(agent) = ExecutorConfigs::get_cached().get_coding_agent(&profile_id) else {
                return Ok(self
                    .mcp_refresh_coordinator
                    .fail(session_id, McpRefreshErrorCategory::MaterializationFailed)
                    .await
                    .unwrap_or(result));
            };
            let servers = match read_coding_agent_mcp_servers(&agent).await {
                Ok(servers) => servers.into_iter().collect(),
                Err(_) => {
                    return Ok(self
                        .mcp_refresh_coordinator
                        .fail(session_id, McpRefreshErrorCategory::MaterializationFailed)
                        .await
                        .unwrap_or(result));
                }
            };
            let snapshot = match validated_mcp_snapshot(BaseCodingAgent::Codex, servers) {
                Ok(snapshot) => snapshot,
                Err(_) => {
                    return Ok(self
                        .mcp_refresh_coordinator
                        .fail(session_id, McpRefreshErrorCategory::MaterializationFailed)
                        .await
                        .unwrap_or(result));
                }
            };
            if snapshot.validate_size().is_err() {
                return Ok(self
                    .mcp_refresh_coordinator
                    .fail(session_id, McpRefreshErrorCategory::MaterializationFailed)
                    .await
                    .unwrap_or(result));
            }
            let coordinator_id = self.cluster_config.coordinator_id.ok_or_else(|| {
                ContainerError::Other(anyhow!("Cluster coordinator identity is missing"))
            })?;
            let request = McpRefreshRequest {
                authority: RequestAuthority {
                    protocol_version: PROTOCOL_VERSION,
                    coordinator_id,
                    worker_node_id: worker_job.worker_node_id,
                    correlation_id: execution.id,
                    issued_at: Utc::now(),
                    nonce: Uuid::new_v4().to_string(),
                },
                execution_id: execution.id,
                snapshot,
            };
            let worker_result = match self.worker_client.as_ref() {
                Some(client) => {
                    client
                        .refresh_mcp(worker_job.worker_node_id, &request)
                        .await
                }
                None => {
                    return Ok(self
                        .mcp_refresh_coordinator
                        .unsupported(session_id)
                        .await
                        .unwrap_or(result));
                }
            };
            let failure = match worker_result {
                Ok(worker_result) => match worker_result.status {
                    WorkerMcpRefreshStatus::Queued => {
                        return Ok(result);
                    }
                    WorkerMcpRefreshStatus::Busy => {
                        return Ok(self
                            .mcp_refresh_coordinator
                            .busy(session_id)
                            .await
                            .unwrap_or(result));
                    }
                    WorkerMcpRefreshStatus::Unsupported => {
                        return Ok(self
                            .mcp_refresh_coordinator
                            .unsupported(session_id)
                            .await
                            .unwrap_or(result));
                    }
                    WorkerMcpRefreshStatus::MaterializationFailed => {
                        McpRefreshErrorCategory::MaterializationFailed
                    }
                    WorkerMcpRefreshStatus::ReloadFailed => McpRefreshErrorCategory::ReloadFailed,
                },
                Err(services::services::cluster::client::WorkerClientError::NotImplemented {
                    ..
                }) => {
                    return Ok(self
                        .mcp_refresh_coordinator
                        .unsupported(session_id)
                        .await
                        .unwrap_or(result));
                }
                Err(_) => McpRefreshErrorCategory::ReloadFailed,
            };
            return Ok(self
                .mcp_refresh_coordinator
                .fail(session_id, failure)
                .await
                .unwrap_or(result));
        }

        let control = self
            .mcp_refresh_controls
            .read()
            .await
            .get(&session_id)
            .map(|(_, handle)| handle.clone());
        if let Some(control) = control
            && let Err(category) = control.0.queue_refresh().await
        {
            return Ok(self
                .mcp_refresh_coordinator
                .fail(session_id, category)
                .await
                .unwrap_or(result));
        }
        // A successful reload is deliberately not confirmed from this
        // execution's control. Codex adopts the queued inventory at the next
        // active-turn boundary; that execution's control is registered by
        // `register_mcp_refresh_control`, which reads the complete status and
        // atomically confirms this pending generation.
        Ok(result)
    }

    async fn mcp_refresh_status(
        &self,
        workspace_id: Uuid,
        session_id: Uuid,
    ) -> Result<Option<McpRefreshResult>, ContainerError> {
        let session = Session::find_by_id(&self.db.pool, session_id)
            .await?
            .ok_or(SessionError::NotFound)?;
        if session.workspace_id != workspace_id {
            return Err(ContainerError::Other(anyhow!(
                "Session does not belong to workspace"
            )));
        }
        Ok(self.mcp_refresh_coordinator.status(session_id).await)
    }

    async fn resolve_org_env_vars(&self, workspace: &Workspace) -> HashMap<String, String> {
        self.resolve_org_env_vars_inner(workspace).await
    }

    async fn touch(&self, workspace: &Workspace) -> Result<(), ContainerError> {
        let now = Instant::now();

        // We debounce touches to avoid excessive database writes, which in SQLites causes DB locks
        let should_debounce = |last_touch: &Instant| -> bool {
            now.duration_since(*last_touch) < WORKSPACE_TOUCH_DEBOUNCE
        };

        // Quick check with read lock
        if self
            .workspace_touch_times
            .read()
            .await
            .get(&workspace.id)
            .is_some_and(should_debounce)
        {
            return Ok(());
        }

        let mut map = self.workspace_touch_times.write().await;
        // Clean up stale entries older than the debounce window, reduce memory usage over time
        map.retain(|_, time| should_debounce(time));
        // check in case another thread has touched already
        if map.get(&workspace.id).is_some_and(should_debounce) {
            return Ok(());
        }
        map.insert(workspace.id, now);
        drop(map);

        Workspace::touch(&self.db.pool, workspace.id).await?;
        Ok(())
    }

    async fn store_db_stream_handle(&self, id: Uuid, handle: JoinHandle<()>) {
        self.add_db_stream_handle(id, handle).await;
    }

    async fn take_db_stream_handle(&self, id: &Uuid) -> Option<JoinHandle<()>> {
        LocalContainerService::take_db_stream_handle(self, id).await
    }

    async fn git_branch_prefix(&self) -> String {
        self.config.read().await.git_branch_prefix.clone()
    }

    fn workspace_to_current_dir(&self, workspace: &Workspace) -> PathBuf {
        PathBuf::from(workspace.container_ref.clone().unwrap_or_default())
    }

    async fn create(&self, workspace: &Workspace) -> Result<ContainerRef, ContainerError> {
        if self.cluster_config.enabled {
            return create_cluster_workspace(self, workspace).await;
        }

        let label = workspace.name.as_deref().unwrap_or("workspace");
        let workspace_dir_name =
            LocalContainerService::dir_name_from_workspace(&workspace.id, label);
        let workspace_dir = WorkspaceManager::get_workspace_base_dir().join(&workspace_dir_name);

        let (repositories, workspace_inputs) = self.workspace_repo_inputs(workspace.id).await?;

        let created_workspace = WorkspaceManager::create_workspace(
            &workspace_dir,
            &workspace_inputs,
            &workspace.branch,
        )
        .await
        .map_err(Self::map_workspace_manager_error)?;

        // Copy project files and images to workspace
        self.copy_files_and_images(&created_workspace.workspace_dir, workspace)
            .await?;

        Self::create_workspace_config_files(&created_workspace.workspace_dir, &repositories)
            .await?;

        Workspace::update_container_ref(
            &self.db.pool,
            workspace.id,
            &created_workspace.workspace_dir.to_string_lossy(),
        )
        .await?;

        Ok(created_workspace
            .workspace_dir
            .to_string_lossy()
            .to_string())
    }

    async fn delete(&self, workspace: &Workspace) -> Result<(), ContainerError> {
        self.try_stop(workspace, true).await;
        self.cleanup_workspace(workspace).await;
        Ok(())
    }

    async fn ensure_container_exists(
        &self,
        workspace: &Workspace,
    ) -> Result<ContainerRef, ContainerError> {
        self.touch(workspace).await?;

        // A process crash after deletion was recorded but before the cleanup
        // fence was released leaves a recoverable `cleaning` row. Repair only
        // that proven-complete state; an in-progress cleanup still has
        // `worktree_deleted = false` and remains fenced.
        if self.cluster_config.enabled && workspace.worktree_deleted {
            WorkspacePlacement::finish_cleanup(&self.db.pool, workspace.id).await?;
        }

        let (repositories, workspace_inputs) = self.workspace_repo_inputs(workspace.id).await?;

        let workspace_dir = if let Some(container_ref) = &workspace.container_ref {
            PathBuf::from(container_ref)
        } else {
            let label = workspace.name.as_deref().unwrap_or("workspace");
            let workspace_dir_name =
                LocalContainerService::dir_name_from_workspace(&workspace.id, label);
            WorkspaceManager::get_workspace_base_dir().join(&workspace_dir_name)
        };

        // Enabling clustering must not change how coordinator-local workspaces
        // behave. `Local` is a terminal, valid placement — it is what every
        // workspace created before clustering has, and what "Automatic
        // placement" yields when no worker is chosen — so it takes the same
        // unfenced local path it took before. Only a workspace actually placed
        // on a worker lives on shared storage and needs the fenced path.
        let cluster_placement = if self.cluster_config.enabled {
            let placement = WorkspacePlacement::find(&self.db.pool, workspace.id)
                .await?
                .ok_or_else(|| ContainerError::Other(anyhow!("Workspace placement is missing")))?;
            Some(placement.placement_state)
        } else {
            None
        };

        match cluster_placement {
            Some(state) if state != WorkspacePlacementState::Local => {
                if state != WorkspacePlacementState::Ready {
                    return Err(ContainerError::Other(anyhow!(
                        "Cluster workspace is not ready (state: {:?})",
                        state
                    )));
                }
                // Heal before ensuring. A workspace created before this change
                // has worktrees pointing at the coordinator's own checkout, and
                // `ensure_workspace_exists_fenced` now administers them in the
                // shared store — where that branch does not yet exist. Left
                // alone it would take the "branch missing" arm, fail to repair
                // linkage it cannot see, and fall through to destructive
                // recreation, deleting exactly the work this change exists to
                // rescue. Adoption re-links the worktree first, so `ensure`
                // then finds a healthy worktree on the expected branch and does
                // nothing.
                if let Some(store) = self.shared_repository_store_for(workspace.id).await? {
                    for input in &workspace_inputs {
                        self.heal_cluster_worktree(
                            &store,
                            &workspace_dir,
                            input,
                            &workspace.branch,
                        )
                        .await;
                    }
                }
                WorkspaceManager::ensure_workspace_exists_fenced(
                    &workspace_dir,
                    &workspace_inputs,
                    &workspace.branch,
                    &self.repository_admin_locks,
                )
                .await
                .map_err(Self::map_workspace_manager_error)?;
            }
            _ => {
                WorkspaceManager::ensure_workspace_exists(
                    &workspace_dir,
                    &workspace_inputs,
                    &workspace.branch,
                )
                .await
                .map_err(Self::map_workspace_manager_error)?;
            }
        }

        if workspace.container_ref.is_none() {
            Workspace::update_container_ref(
                &self.db.pool,
                workspace.id,
                &workspace_dir.to_string_lossy(),
            )
            .await?;
        }

        if workspace.worktree_deleted {
            Workspace::clear_worktree_deleted(&self.db.pool, workspace.id).await?;
        }

        // Copy project files and images (fast no-op if already exist)
        self.copy_files_and_images(&workspace_dir, workspace)
            .await?;

        Self::create_workspace_config_files(&workspace_dir, &repositories).await?;

        Ok(workspace_dir.to_string_lossy().to_string())
    }

    async fn is_container_clean(&self, workspace: &Workspace) -> Result<bool, ContainerError> {
        let Some(container_ref) = &workspace.container_ref else {
            return Ok(true);
        };

        let workspace_dir = PathBuf::from(container_ref);
        if !workspace_dir.exists() {
            return Ok(true);
        }

        let repositories =
            WorkspaceRepo::find_repos_for_workspace(&self.db.pool, workspace.id).await?;

        for repo in &repositories {
            let worktree_path = workspace_dir.join(&repo.name);
            if worktree_path.exists() {
                let (uncommitted, untracked) =
                    self.git().get_worktree_change_counts(&worktree_path)?;
                if uncommitted > 0 || untracked > 0 {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    async fn dispatch_execution(
        &self,
        workspace: &Workspace,
        execution_process: &ExecutionProcess,
        executor_action: &ExecutorAction,
    ) -> Result<(), ContainerError> {
        if !self.cluster_config.enabled {
            return self
                .start_execution_inner(workspace, execution_process, executor_action)
                .await;
        }

        let placement = WorkspacePlacement::find(&self.db.pool, workspace.id)
            .await?
            .ok_or_else(|| ContainerError::Other(anyhow!("Workspace placement is missing")))?;
        // A coordinator-local workspace executes locally whether or not this
        // node is a cluster coordinator. Without this, turning clustering on
        // made every pre-existing workspace unrunnable.
        if placement.placement_state == WorkspacePlacementState::Local {
            return self
                .start_execution_inner(workspace, execution_process, executor_action)
                .await;
        }
        if placement.placement_state != WorkspacePlacementState::Ready {
            return Err(ContainerError::Other(anyhow!(
                "Cluster workspace is not ready for execution (state: {:?})",
                placement.placement_state
            )));
        }
        let worker_node_id = placement.worker_node_id.ok_or_else(|| {
            ContainerError::Other(anyhow!("Ready cluster workspace has no assigned worker"))
        })?;
        let coordinator_id = self.cluster_config.coordinator_id.ok_or_else(|| {
            ContainerError::Other(anyhow!("Cluster coordinator identity is missing"))
        })?;
        let client = self.worker_client.as_ref().ok_or_else(|| {
            ContainerError::Other(anyhow!("Cluster worker client is not configured"))
        })?;
        let workspace_path = workspace.container_ref.clone().ok_or_else(|| {
            ContainerError::Other(anyhow!("Container ref not found for workspace"))
        })?;

        let mut environment = BTreeMap::new();
        environment.extend(self.resolve_org_env_vars(workspace).await);
        // Send only workspace-relative Go state. The worker must construct PATH
        // from its own supervised environment; coordinator Nix store paths are
        // not necessarily valid on a heterogeneous worker.
        environment.extend(workspace_go_environment(Path::new(&workspace_path), None));
        environment.insert("VK_WORKSPACE_ID".into(), workspace.id.to_string());
        environment.insert("VK_WORKSPACE_BRANCH".into(), workspace.branch.clone());

        let executor_config = match executor_action.typ() {
            ExecutorActionType::CodingAgentInitialRequest(request) => {
                Some(&request.executor_config)
            }
            ExecutorActionType::CodingAgentFollowUpRequest(request) => {
                Some(&request.executor_config)
            }
            ExecutorActionType::ReviewRequest(request) => Some(&request.executor_config),
            ExecutorActionType::ScriptRequest(_) => None,
        };
        let executor_profile = executor_config
            .map(|config| config.profile_id().to_string())
            .unwrap_or_else(|| "script".into());
        // Send the definition, not just the name. A variant is user-defined data
        // that lives only here; a worker holds the embedded defaults, which
        // define DEFAULT and nothing else.
        let executor_profile_config = executor_config
            .and_then(dispatched_executor_profile)
            .map(serde_json::to_value)
            .transpose()
            .map_err(anyhow::Error::from)?;
        let mcp_config_snapshot = if let Some(config) = executor_config {
            let profile_id = config.profile_id();
            let agent = ExecutorConfigs::get_cached()
                .get_coding_agent(&profile_id)
                .ok_or_else(|| anyhow!("executor profile {profile_id} is unavailable"))?;
            if agent.supports_mcp() {
                let snapshot = validated_mcp_snapshot(
                    config.executor,
                    read_coding_agent_mcp_servers(&agent)
                        .await
                        .map_err(anyhow::Error::from)?
                        .into_iter()
                        .collect(),
                )?;
                Some(snapshot)
            } else {
                None
            }
        } else {
            None
        };
        let action = serde_json::to_value(executor_action).map_err(anyhow::Error::from)?;
        let run_reason = serde_json::to_value(&execution_process.run_reason)
            .map_err(anyhow::Error::from)?
            .as_str()
            .unwrap_or("unknown")
            .to_owned();
        let persistence = if execution_process.run_reason.is_persistent() {
            PersistencePolicy::Persistent
        } else {
            PersistencePolicy::Ordinary
        };
        let digest_material = serde_json::to_vec(&json!({
            "execution_id": execution_process.id,
            "workspace_id": workspace.id,
            "session_id": execution_process.session_id,
            "worker_node_id": worker_node_id,
            "workspace_path": workspace_path,
            "executor_profile": executor_profile,
            // Part of the request's identity: the worker treats a dispatch with
            // a different digest as a different request. A profile edited
            // between two dispatches of the same execution must not be deduped
            // into the first one's definition.
            "executor_profile_config": executor_profile_config,
            "mcp_config_snapshot": mcp_config_snapshot,
            "action": action,
            "environment": environment,
            "run_reason": run_reason,
            "persistence": persistence,
        }))
        .map_err(anyhow::Error::from)?;
        let request_digest = format!("sha256:{:x}", Sha256::digest(digest_material));
        let authority = RequestAuthority {
            protocol_version: PROTOCOL_VERSION,
            coordinator_id,
            worker_node_id,
            correlation_id: execution_process.id,
            issued_at: Utc::now(),
            nonce: Uuid::new_v4().to_string(),
        };
        let dispatch = ExecutionDispatch {
            authority,
            execution_id: execution_process.id,
            workspace_id: workspace.id,
            session_id: execution_process.session_id,
            workspace_path: workspace_path.clone(),
            working_directory: workspace_path,
            executor_profile,
            executor_profile_config,
            mcp_config_snapshot,
            action,
            environment,
            run_reason,
            timeout_seconds: None,
            persistence,
            request_digest: request_digest.clone(),
        };

        ExecutionWorkerJob::create_pending(
            &self.db.pool,
            execution_process.id,
            worker_node_id,
            &request_digest,
        )
        .await?;
        let accepted = match client.dispatch(worker_node_id, &dispatch).await {
            Ok(accepted) => accepted,
            Err(error) => {
                let evidence = serde_json::json!({
                    "reason": "worker dispatch failed",
                    "error": error.to_string(),
                });
                ExecutionWorkerJob::update_state(
                    &self.db.pool,
                    execution_process.id,
                    ExecutionWorkerDispatchState::Failed,
                    Some(&evidence),
                    Some(Utc::now()),
                )
                .await?;
                return Err(ContainerError::Other(anyhow!(error)));
            }
        };
        if accepted.execution_id != execution_process.id
            || accepted.request_digest != request_digest
        {
            let evidence = serde_json::json!({
                "reason": "worker returned mismatched dispatch acceptance",
            });
            ExecutionWorkerJob::update_state(
                &self.db.pool,
                execution_process.id,
                ExecutionWorkerDispatchState::Failed,
                Some(&evidence),
                Some(Utc::now()),
            )
            .await?;
            return Err(ContainerError::Other(anyhow!(
                "Worker returned mismatched dispatch acceptance"
            )));
        }
        if !ExecutionWorkerJob::record_acceptance(
            &self.db.pool,
            execution_process.id,
            accepted.worker_job_id,
            accepted.last_sequence as i64,
        )
        .await?
        {
            return Err(ContainerError::Other(anyhow!(
                "Execution worker job was not pending during acceptance"
            )));
        }
        if dispatch.mcp_config_snapshot.is_some()
            && self
                .mcp_refresh_coordinator
                .status(execution_process.session_id)
                .await
                .is_some_and(|state| {
                    state.status == McpRefreshStatus::PendingNextTurn
                        && state.requested_at <= execution_process.started_at
                })
        {
            let client = client.clone();
            let coordinator = self.mcp_refresh_coordinator.clone();
            let session_id = execution_process.session_id;
            let execution_id = execution_process.id;
            let snapshot = dispatch.mcp_config_snapshot.clone().expect("checked above");
            tokio::spawn(async move {
                for _ in 0..30 {
                    let request = McpRefreshRequest {
                        authority: RequestAuthority {
                            protocol_version: PROTOCOL_VERSION,
                            coordinator_id,
                            worker_node_id,
                            correlation_id: execution_id,
                            issued_at: Utc::now(),
                            nonce: Uuid::new_v4().to_string(),
                        },
                        execution_id,
                        snapshot: snapshot.clone(),
                    };
                    if let Ok(status) = client.mcp_status(worker_node_id, &request).await
                        && status.status == WorkerMcpRefreshStatus::Queued
                    {
                        let servers = status
                            .servers
                            .into_iter()
                            .filter_map(|server| serde_json::from_value(server).ok())
                            .collect();
                        coordinator.confirm(session_id, servers).await;
                        return;
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                coordinator
                    .fail(session_id, McpRefreshErrorCategory::Timeout)
                    .await;
            });
        }
        self.track_worker_msgs_in_store(execution_process, worker_node_id)
            .await?;
        Ok(())
    }

    async fn start_execution_inner(
        &self,
        workspace: &Workspace,
        execution_process: &ExecutionProcess,
        executor_action: &ExecutorAction,
    ) -> Result<(), ContainerError> {
        // Get the worktree path
        let container_ref = workspace
            .container_ref
            .as_ref()
            .ok_or(ContainerError::Other(anyhow!(
                "Container ref not found for workspace"
            )))?;
        let current_dir = PathBuf::from(container_ref);

        let approvals_service: Arc<dyn ExecutorApprovalService> =
            match executor_action.base_executor() {
                Some(
                    BaseCodingAgent::Codex
                    | BaseCodingAgent::ClaudeCode
                    | BaseCodingAgent::Gemini
                    | BaseCodingAgent::QwenCode
                    | BaseCodingAgent::Opencode
                    | BaseCodingAgent::Grok,
                ) => ExecutorApprovalBridge::new(
                    self.approvals.clone(),
                    self.db.clone(),
                    self.notification_service.clone(),
                    execution_process.id,
                ),
                _ => Arc::new(NoopExecutorApprovalService {}),
            };

        let repos = WorkspaceRepo::find_repos_for_workspace(&self.db.pool, workspace.id).await?;
        let repo_names: Vec<String> = repos.iter().map(|r| r.name.clone()).collect();
        let repo_context = RepoContext::new(current_dir.clone(), repo_names);

        let config = self.config.read().await;
        let commit_reminder_enabled = config.commit_reminder_enabled;
        let commit_reminder_prompt = config
            .commit_reminder_prompt
            .clone()
            .unwrap_or_else(|| DEFAULT_COMMIT_REMINDER_PROMPT.to_string());
        drop(config);
        let mut env = ExecutionEnv::new(
            repo_context,
            commit_reminder_enabled,
            commit_reminder_prompt,
        );

        // Inject organization-level env vars (e.g. GITHUB_TOKEN) for workspaces
        // linked to a remote org project. Injected here — the single point every
        // execution (initial, setup, follow-up) flows through — so all agent
        // processes in the workspace receive them. Applied BEFORE the VK_* context
        // below so an org var can never clobber the internal workspace contract.
        // Best-effort: on any failure we log and start without them rather than
        // block the workspace.
        let org_env_vars = self.resolve_org_env_vars(workspace).await;
        if !org_env_vars.is_empty() {
            env.merge(&org_env_vars);
        }
        let inherited_path = std::env::var_os("PATH").unwrap_or_default();
        env.merge(&workspace_go_environment(
            &current_dir,
            Some(&inherited_path),
        ));

        // Always inject workspace/session context (wins over any org var above).
        env.insert("VK_WORKSPACE_ID", workspace.id.to_string());
        env.insert("VK_WORKSPACE_BRANCH", &workspace.branch);

        // Expose app-installed CLI tools (services::cli_tools) to agents.
        // Appended after the inherited PATH so a host-provided copy of the
        // same tool always wins over an app-installed one. PATH is a reserved
        // env name (org vars can't set it), but base the merge on any PATH
        // already in the env so this stays correct if that ever changes.
        let inherited = env
            .get("PATH")
            .map(std::ffi::OsString::from)
            .unwrap_or_else(|| std::env::var_os("PATH").unwrap_or_default());
        if let Some(merged) = utils::shell::append_cli_tools_to_path(&inherited) {
            env.insert("PATH", merged.to_string_lossy().into_owned());
        }

        // Persistent processes (dev servers, background helpers) write their
        // output straight to a raw log file (instead of pipes) so they can
        // keep running across a server restart; the server tails the file.
        // Unix only: adoption after a restart relies on process-group
        // management.
        #[cfg(unix)]
        let dev_server_raw_log = if execution_process.run_reason.is_persistent() {
            let path = utils::execution_logs::process_raw_log_file_path(
                execution_process.session_id,
                execution_process.id,
            );
            if let Some(parent) = path.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            env.insert(
                executors::actions::script::RAW_LOG_PATH_ENV,
                path.to_string_lossy().into_owned(),
            );
            Some(path)
        } else {
            None
        };
        #[cfg(not(unix))]
        let dev_server_raw_log: Option<PathBuf> = None;

        // Phase 2 warm reuse: when the gate is on and this is an OpenCode
        // follow-up whose session has a *live* warm server parked, drive the turn
        // against that server instead of spawning a new process. A miss or a
        // died-out-of-band entry reaps itself and falls through to a normal cold
        // start. The `SpawnedChild` from either path flows through the identical
        // pipeline below (pgid record, msg tracking, exit monitor).
        let warm_reuse_hit = if self.warm_agents_enabled()
            && executor_action.base_executor() == Some(BaseCodingAgent::Opencode)
            && matches!(
                executor_action.typ(),
                ExecutorActionType::CodingAgentFollowUpRequest(_)
            ) {
            self.take_live_warm_server(&execution_process.session_id)
                .await
        } else {
            None
        };

        // Create the child and stream, add to execution tracker with timeout.
        // On a warm-reuse hit, try the warm path first; if it errors (e.g. the
        // warm server failed a health check), fall back to a normal cold start
        // rather than failing the whole turn — `spawn_warm_follow_up` has already
        // killed the warm server's process group on that error path, so a fresh
        // server is cleanly spawned with no orphan.
        let warm_spawned = if let Some((warm_child, reuse)) = warm_reuse_hit {
            let ExecutorActionType::CodingAgentFollowUpRequest(follow_up) = executor_action.typ()
            else {
                unreachable!("warm reuse is only taken for follow-up requests")
            };
            tracing::info!(
                "Reusing warm OpenCode server for session {}",
                execution_process.session_id
            );
            match follow_up
                .spawn_warm_follow_up(
                    warm_child,
                    &reuse,
                    &current_dir,
                    approvals_service.clone(),
                    &env,
                )
                .await
            {
                Ok(spawned) => Some(spawned),
                Err(e) => {
                    tracing::warn!(
                        "Warm reuse failed for session {}, cold-starting instead: {e}",
                        execution_process.session_id
                    );
                    None
                }
            }
        } else {
            None
        };

        let spawn_result: Result<_, ContainerError> = match warm_spawned {
            Some(spawned) => Ok(spawned),
            None => match tokio::time::timeout(
                Duration::from_secs(30),
                executor_action.spawn(&current_dir, approvals_service, &env),
            )
            .await
            {
                Ok(result) => result.map_err(ContainerError::ExecutorError),
                Err(_) => Err(ContainerError::Other(anyhow!(
                    "Timeout: process took more than 30 seconds to start"
                ))),
            },
        };
        let mut spawned = match spawn_result {
            Ok(spawned) => spawned,
            Err(error) => {
                if matches!(
                    execution_process.run_reason,
                    ExecutionProcessRunReason::CodingAgent
                ) && self
                    .mcp_refresh_coordinator
                    .status(execution_process.session_id)
                    .await
                    .is_some_and(|state| state.status == McpRefreshStatus::PendingNextTurn)
                {
                    self.mcp_refresh_coordinator
                        .fail(
                            execution_process.session_id,
                            McpRefreshErrorCategory::ProcessLaunchFailed,
                        )
                        .await;
                }
                return Err(error);
            }
        };

        // Persistent app-servers ask to be kept warm across turns: on a clean
        // turn-completion signal the exit monitor ends the turn without killing
        // the process group. The container is the gate authority — an executor's
        // declared capability is only honored when the env gate is on (spec
        // FR-8), so default behavior is unchanged. Captured before `spawned` is
        // consumed below, along with the reuse handle the executor surfaces.
        let keep_warm = spawned.keep_warm && self.warm_agents_enabled();
        let warm_reuse = spawned.warm_reuse.take();
        if let Some(signal) = spawned.mcp_refresh.take() {
            self.register_mcp_refresh_control(
                execution_process.session_id,
                execution_process.id,
                execution_process.started_at,
                signal,
            );
        } else if matches!(
            execution_process.run_reason,
            ExecutionProcessRunReason::CodingAgent
        ) && self
            .mcp_refresh_coordinator
            .status(execution_process.session_id)
            .await
            .is_some_and(|state| state.status == McpRefreshStatus::PendingNextTurn)
        {
            self.mcp_refresh_coordinator
                .fail(
                    execution_process.session_id,
                    McpRefreshErrorCategory::Unsupported,
                )
                .await;
        }

        // Record the process group id (== leader pid for grouped spawns) so a
        // later boot can clean up the group if this server dies uncleanly.
        if let Some(pid) = spawned.child.id()
            && let Err(e) =
                ExecutionProcess::update_pgid(&self.db.pool, execution_process.id, pid as i64).await
        {
            tracing::warn!(
                "Failed to record pgid for execution process {}: {}",
                execution_process.id,
                e
            );
        }

        if let Some(path) = dev_server_raw_log {
            self.track_raw_file_msgs_in_store(execution_process.id, path)
                .await;
        } else {
            self.track_child_msgs_in_store(execution_process.id, &mut spawned.child)
                .await;
        }

        self.add_child_to_store(execution_process.id, spawned.child)
            .await;

        // Store cancellation token for graceful shutdown
        if let Some(cancel) = spawned.cancel {
            self.add_cancellation_token(execution_process.id, cancel)
                .await;
        }

        // Spawn unified exit monitor: watches OS exit and optional executor
        // signal. When kept warm, the monitor moves the child into the warm
        // registry keyed by this session, using `warm_reuse` as the handle.
        let hn = self.spawn_exit_monitor(
            &execution_process.id,
            execution_process.session_id,
            spawned.exit_signal,
            keep_warm,
            warm_reuse,
        );
        self.add_exit_monitor_handle(execution_process.id, hn).await;

        Ok(())
    }

    async fn stop_execution(
        &self,
        execution_process: &ExecutionProcess,
        status: ExecutionProcessStatus,
    ) -> Result<(), ContainerError> {
        // Explicitly stopping any execution in a session also reaps that
        // session's warm app-server, if one is parked (spec FR-4a). A warm
        // process has no running turn so it is not the `child` below — reap it
        // by session key. Idempotent no-op when there is none.
        self.reap_warm_server(&execution_process.session_id).await;

        if let Some(worker_job) =
            ExecutionWorkerJob::find_by_execution_id(&self.db.pool, execution_process.id).await?
        {
            let coordinator_id = self.cluster_config.coordinator_id.ok_or_else(|| {
                ContainerError::Other(anyhow!("Cluster coordinator identity is missing"))
            })?;
            let client = self.worker_client.as_ref().ok_or_else(|| {
                ContainerError::Other(anyhow!("Cluster worker client is not configured"))
            })?;
            let request = CancellationRequest {
                authority: RequestAuthority {
                    protocol_version: PROTOCOL_VERSION,
                    coordinator_id,
                    worker_node_id: worker_job.worker_node_id,
                    correlation_id: execution_process.id,
                    issued_at: Utc::now(),
                    nonce: Uuid::new_v4().to_string(),
                },
                execution_id: execution_process.id,
                graceful_timeout_seconds: 5,
                terminate_timeout_seconds: 5,
            };
            match client.cancel(worker_job.worker_node_id, &request).await {
                Ok(response)
                    if matches!(
                        response.phase,
                        CancellationPhase::Confirmed | CancellationPhase::AlreadyTerminal
                    ) && response.terminal.is_some() =>
                {
                    let evidence = response.terminal.expect("guarded above");
                    let (worker_state, process_state) = match evidence.state {
                        TerminalState::Completed => (
                            ExecutionWorkerDispatchState::Completed,
                            ExecutionProcessStatus::Completed,
                        ),
                        TerminalState::Failed => (
                            ExecutionWorkerDispatchState::Failed,
                            ExecutionProcessStatus::Failed,
                        ),
                        TerminalState::Killed => (
                            ExecutionWorkerDispatchState::Killed,
                            ExecutionProcessStatus::Killed,
                        ),
                        TerminalState::Interrupted => (
                            ExecutionWorkerDispatchState::Interrupted,
                            ExecutionProcessStatus::Interrupted,
                        ),
                    };
                    let evidence_json = serde_json::to_value(&evidence).ok();
                    ExecutionWorkerJob::update_state(
                        &self.db.pool,
                        execution_process.id,
                        worker_state,
                        evidence_json.as_ref(),
                        Some(evidence.observed_at),
                    )
                    .await?;
                    ExecutionProcess::update_completion(
                        &self.db.pool,
                        execution_process.id,
                        process_state,
                        evidence.exit_code.map(i64::from),
                    )
                    .await?;
                }
                Ok(response) => {
                    tracing::warn!(
                        execution_id = %execution_process.id,
                        phase = ?response.phase,
                        "Worker did not confirm a terminal state after cancellation"
                    );
                    mark_remote_execution_indeterminate(&self.db, execution_process.id).await?;
                }
                Err(error) => {
                    tracing::warn!(
                        execution_id = %execution_process.id,
                        "Remote cancellation could not be confirmed: {error}"
                    );
                    mark_remote_execution_indeterminate(&self.db, execution_process.id).await?;
                }
            }
            self.finish_msg_store(&execution_process.id).await;
            return Ok(());
        }

        let Some(child) = self.get_child_from_store(&execution_process.id).await else {
            // No in-memory handle: the process may have been adopted from a
            // previous server instance and is managed by pgid only.
            if let Some(pgid) = self.take_adopted_pgid(&execution_process.id).await {
                return self
                    .stop_adopted_execution(execution_process, status, pgid)
                    .await;
            }
            // Deliberately returns before `update_completion`, leaving the row
            // `Running`. That is load-bearing, not an oversight: the next boot's
            // `cleanup_orphan_executions` selects rows via `find_running` and
            // unconditionally snapshots their uncommitted work. Marking the row
            // terminal here would hide it from that sweep and remove the last
            // safety net for a session whose child vanished with the server.
            return Err(ContainerError::Other(anyhow!(
                "Child process not found for execution"
            )));
        };
        let exit_code = if status == ExecutionProcessStatus::Completed {
            Some(0)
        } else {
            None
        };

        ExecutionProcess::update_completion(&self.db.pool, execution_process.id, status, exit_code)
            .await?;

        // Try graceful cancellation first, then force kill
        if let Some(cancel) = self.take_cancellation_token(&execution_process.id).await {
            cancel.cancel();

            // Wait for exit monitor to finish gracefully
            if let Some(monitor_handle) = self.take_exit_monitor_handle(&execution_process.id).await
            {
                match tokio::time::timeout(Duration::from_secs(5), monitor_handle).await {
                    Ok(_) => {
                        tracing::debug!("Process {} exited gracefully", execution_process.id);
                    }
                    Err(_) => {
                        tracing::debug!(
                            "Graceful shutdown timed out for process {}, force killing",
                            execution_process.id
                        );
                    }
                }
            }
        }

        {
            let mut child_guard = child.write().await;
            if let Err(e) = command::kill_process_group(&mut child_guard).await {
                tracing::error!(
                    "Failed to stop execution process {}: {}",
                    execution_process.id,
                    e
                );
                return Err(e);
            }
        }
        self.remove_child_from_store(&execution_process.id).await;

        // Mark the process finished in the MsgStore and wait for DB persistence
        self.finish_raw_log_tailer(&execution_process.id).await;
        let db_stream_handle = self.take_db_stream_handle(&execution_process.id).await;
        self.finish_msg_store(&execution_process.id).await;
        if let Some(handle) = db_stream_handle {
            let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        }

        tracing::debug!(
            "Execution process {} stopped successfully",
            execution_process.id
        );

        // Record after-head commit OID (best-effort)
        self.update_after_head_commits(execution_process.id).await;

        Ok(())
    }

    async fn stream_diff(
        &self,
        workspace: &Workspace,
        stats_only: bool,
    ) -> Result<futures::stream::BoxStream<'static, Result<LogMsg, std::io::Error>>, ContainerError>
    {
        let workspace_repos =
            WorkspaceRepo::find_by_workspace_id(&self.db.pool, workspace.id).await?;
        let target_branches: HashMap<_, _> = workspace_repos
            .iter()
            .map(|wr| (wr.repo_id, wr.target_branch.clone()))
            .collect();

        let repositories =
            WorkspaceRepo::find_repos_for_workspace(&self.db.pool, workspace.id).await?;

        let mut streams = Vec::new();

        let container_ref = self.ensure_container_exists(workspace).await?;
        let workspace_root = PathBuf::from(container_ref);

        for repo in repositories {
            let worktree_path = workspace_root.join(&repo.name);
            let branch = &workspace.branch;

            let Some(target_branch) = target_branches.get(&repo.id) else {
                tracing::warn!(
                    "Skipping diff stream for repo {}: no target branch configured",
                    repo.name
                );
                continue;
            };

            let base_commit = match self
                .git()
                .get_base_commit(&repo.path, branch, target_branch)
            {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        "Skipping diff stream for repo {}: failed to get base commit: {}",
                        repo.name,
                        e
                    );
                    continue;
                }
            };

            let stream = self
                .create_live_diff_stream(diff_stream::DiffStreamArgs {
                    git_service: self.git().clone(),
                    db: self.db().clone(),
                    workspace_id: workspace.id,
                    repo_id: repo.id,
                    repo_path: repo.path.clone(),
                    worktree_path: worktree_path.clone(),
                    branch: branch.to_string(),
                    target_branch: target_branch.clone(),
                    base_commit: base_commit.clone(),
                    stats_only,
                    path_prefix: Some(repo.name.clone()),
                })
                .await?;

            streams.push(Box::pin(stream));
        }

        if streams.is_empty() {
            return Ok(Box::pin(futures::stream::empty()));
        }

        // Merge all streams into one
        Ok(Box::pin(futures::stream::select_all(streams)))
    }

    async fn try_commit_changes(&self, ctx: &ExecutionContext) -> Result<bool, ContainerError> {
        if !matches!(
            ctx.execution_process.run_reason,
            ExecutionProcessRunReason::CodingAgent | ExecutionProcessRunReason::CleanupScript,
        ) {
            return Ok(false);
        }

        let message = self.get_commit_message(ctx).await;

        let container_ref = ctx
            .workspace
            .container_ref
            .as_ref()
            .ok_or_else(|| ContainerError::Other(anyhow!("Container reference not found")))?;
        let workspace_root = PathBuf::from(container_ref);

        let repos_with_changes = self.check_repos_for_changes(&workspace_root, &ctx.repos)?;
        if repos_with_changes.is_empty() {
            tracing::debug!("No changes to commit in any repository");
            return Ok(false);
        }

        Ok(self.commit_repos(repos_with_changes, &message))
    }

    async fn commit_interrupted_wip(
        &self,
        process: &ExecutionProcess,
    ) -> Result<(), ContainerError> {
        if !matches!(
            process.run_reason,
            ExecutionProcessRunReason::CodingAgent | ExecutionProcessRunReason::CleanupScript
        ) {
            return Ok(());
        }

        let ctx = ExecutionProcess::load_context(&self.db.pool, process.id).await?;
        let container_ref = ctx.workspace.container_ref.as_ref().ok_or_else(|| {
            ContainerError::Other(anyhow!(
                "Container reference missing for interrupted process {}",
                process.id
            ))
        })?;

        let workspace_root = PathBuf::from(container_ref);
        let repos_with_changes = self.check_repos_for_changes(&workspace_root, &ctx.repos)?;
        let mut failures = Vec::new();
        for (repo, worktree_path) in repos_with_changes {
            match self.git().commit(
                &worktree_path,
                "WIP: run interrupted by vibe-kanban shutdown",
            ) {
                Ok(true) => tracing::info!("Committed interrupted WIP in repo '{}'", repo.name),
                Ok(false) => {
                    failures.push(format!(
                        "repo '{}': interrupted WIP snapshot produced no commit",
                        repo.name
                    ));
                }
                Err(e) => {
                    failures.push(format!("repo '{}': {}", repo.name, e));
                }
            }
        }

        // Always record the resulting HEADs, including partial success in a
        // multi-repo workspace, so durable WIP commits are never omitted from
        // this process's repository state.
        self.update_after_head_commits(process.id).await;

        if !failures.is_empty() {
            return Err(ContainerError::Other(anyhow!(
                "Failed to capture interrupted WIP: {}",
                failures.join("; ")
            )));
        }

        Ok(())
    }

    /// Copy files from the original project directory to the worktree.
    /// Skips files that already exist at target with same size.
    async fn copy_project_files(
        &self,
        source_dir: &Path,
        target_dir: &Path,
        copy_files: &str,
    ) -> Result<(), ContainerError> {
        let source_dir = source_dir.to_path_buf();
        let target_dir = target_dir.to_path_buf();
        let copy_files = copy_files.to_string();

        tokio::time::timeout(
            std::time::Duration::from_secs(30),
            tokio::task::spawn_blocking(move || {
                copy::copy_project_files_impl(&source_dir, &target_dir, &copy_files)
            }),
        )
        .await
        .map_err(|_| ContainerError::Other(anyhow!("Copy project files timed out after 30s")))?
        .map_err(|e| ContainerError::Other(anyhow!("Copy files task failed: {e}")))?
    }

    async fn try_adopt_execution(&self, process: &ExecutionProcess) -> bool {
        #[cfg(not(unix))]
        {
            let _ = process;
            false
        }
        #[cfg(unix)]
        {
            if !process.run_reason.is_persistent() {
                return false;
            }
            let Some(pgid) = process.pgid else {
                return false;
            };
            // A raw log file means the process writes its own output and can
            // be tailed; without one (e.g. spawned by an older version with
            // piped output) adoption is impossible.
            let raw_log_path =
                utils::execution_logs::process_raw_log_file_path(process.session_id, process.id);
            if !raw_log_path.exists() {
                return false;
            }
            let age_secs = (chrono::Utc::now() - process.started_at).num_seconds();
            if !utils::process::process_group_leader_matches(pgid as i32, age_secs).await {
                return false;
            }

            self.adopted_pgids
                .write()
                .await
                .insert(process.id, pgid as i32);
            self.track_raw_file_msgs_in_store(process.id, raw_log_path)
                .await;
            let watcher = self.spawn_adopted_exit_watcher(process.id, pgid as i32);
            self.add_exit_monitor_handle(process.id, watcher).await;
            tracing::info!(
                "Adopted running {:?} process {} (pgid {})",
                process.run_reason,
                process.id,
                pgid
            );
            true
        }
    }

    async fn kill_all_running_processes(&self) -> Result<(), ContainerError> {
        tracing::info!("Killing all running processes");

        // Parked warm app-servers are `Completed` rows, so `find_running()` below
        // does not see them, and (being non-persistent `CodingAgent`s) they are
        // not in the boot re-adoption path — reap them explicitly on shutdown or
        // they orphan across a restart (spec FR-4c).
        self.reap_all_warm_servers().await;

        let running_processes = ExecutionProcess::find_running(&self.db.pool).await?;

        tracing::info!(
            "Found {} running processes to kill",
            running_processes.len()
        );

        for process in running_processes {
            // On unix, persistent processes (dev servers, background helpers)
            // are detached (their output goes to a raw log file) and are left
            // running across the restart; the next boot re-adopts them via
            // their process group id.
            #[cfg(unix)]
            if process.run_reason.is_persistent() {
                self.detach_execution_for_handoff(&process).await;
                continue;
            }

            tracing::info!(
                "Killing process: id={}, run_reason={:?}",
                process.id,
                process.run_reason
            );
            // Mark as interrupted (not killed): the process is stopped by a
            // server shutdown/restart, so the run can be offered for resume.
            match self
                .stop_execution(&process, ExecutionProcessStatus::Interrupted)
                .await
            {
                Ok(()) => tracing::info!("Successfully killed process: id={}", process.id),
                Err(error) => tracing::error!(
                    "Failed to cleanly kill running execution process {:?}: {:?}",
                    process,
                    error
                ),
            }

            // Preserve the work whether or not the kill succeeded. Stopping the
            // process and saving its output are independent concerns, and the
            // failure case is the one most likely to leave unsaved work on disk:
            // `stop_execution` reports "child process not found" whenever the
            // child already died with (or just before) the server, which is the
            // ordinary shape of a restart. Gating preservation on a clean kill
            // therefore skipped it in exactly the situation it exists for.
            // Kept after the stop attempt (never before it) so a live writer has
            // been signalled first.
            if let Err(error) = self.commit_interrupted_wip(&process).await {
                tracing::error!(
                    "Failed to preserve interrupted process {} work for session {}; \
                     uncommitted changes may be at risk: {}",
                    process.id,
                    process.session_id,
                    error
                );
            }
        }

        Ok(())
    }
}

/// Fail unless every worktree in `workspace_dir` is backed by a repository that
/// resolves inside `shared_root`.
///
/// Enumerates all of them and reports every violation, rather than stopping at
/// the first: an operator fixing a multi-repository workspace needs the whole
/// list, not one entry at a time. `Indeterminate` counts as a violation here
/// because this runs before a workspace is advertised as ready, and "could not
/// tell" is not a basis for telling a user their workspace works.
fn assert_worktrees_are_portable(
    shared_root: &Path,
    workspace_dir: &Path,
    inputs: &[RepoWorkspaceInput],
) -> Result<(), ContainerError> {
    let mut violations = Vec::new();
    for input in inputs {
        let worktree_path = workspace_dir.join(&input.repo.name);
        let status = WorktreeLinkage::probe(&worktree_path, shared_root);
        if !status.is_portable() {
            violations.push(format!("{}: {}", input.repo.name, status.describe()));
        }
    }
    if violations.is_empty() {
        return Ok(());
    }
    Err(ContainerError::Other(anyhow!(
        "worktrees are not usable from other nodes: {}",
        violations.join("; ")
    )))
}

async fn create_cluster_workspace(
    service: &LocalContainerService,
    workspace: &Workspace,
) -> Result<ContainerRef, ContainerError> {
    let placement = WorkspacePlacement::find(&service.db.pool, workspace.id)
        .await?
        .ok_or_else(|| ContainerError::Other(anyhow!("Workspace placement is missing")))?;
    if placement.placement_state != WorkspacePlacementState::Reserved
        || placement.worker_node_id.is_none()
    {
        return Err(ContainerError::Other(anyhow!(
            "Workspace must have a reserved worker before provisioning"
        )));
    }
    if !WorkspacePlacement::transition(
        &service.db.pool,
        workspace.id,
        WorkspacePlacementState::Reserved,
        WorkspacePlacementState::Provisioning,
        None,
    )
    .await?
    {
        return Err(ContainerError::Other(anyhow!(
            "Workspace placement changed before provisioning"
        )));
    }

    let result = async {
        let paths = SharedWorkspacePaths::new(&service.cluster_config.shared_root)
            .map_err(LocalContainerService::map_workspace_manager_error)?;
        paths.create_base_dirs().await?;
        let workspace_dir = paths.workspace_dir(workspace.id);
        let (repositories, workspace_inputs) = service.workspace_repo_inputs(workspace.id).await?;

        // Materialise the shared store for every repository *before* any
        // worktree is created from it. Without this the worktree would be
        // created from the coordinator's own checkout and record a path no
        // worker can resolve — the defect this exists to close.
        let store = service
            .shared_repository_store_for(workspace.id)
            .await?
            .ok_or_else(|| {
                ContainerError::Other(anyhow!(
                    "Cluster provisioning requires a shared repository store"
                ))
            })?;
        for input in &workspace_inputs {
            store
                .ensure(&input.repo, &input.target_branch)
                .await
                .map_err(LocalContainerService::map_workspace_manager_error)?;
        }

        let created_workspace = WorkspaceManager::create_workspace_fenced(
            &workspace_dir,
            &workspace_inputs,
            &workspace.branch,
            &service.repository_admin_locks,
        )
        .await
        .map_err(LocalContainerService::map_workspace_manager_error)?;

        // Assert portability before this workspace can be advertised as ready.
        // A workspace whose worktrees a worker cannot use must fail loudly here,
        // not silently several minutes into an agent's first turn.
        assert_worktrees_are_portable(
            paths.root(),
            &created_workspace.workspace_dir,
            &workspace_inputs,
        )?;

        service
            .copy_files_and_images(&created_workspace.workspace_dir, workspace)
            .await?;
        LocalContainerService::create_workspace_config_files(
            &created_workspace.workspace_dir,
            &repositories,
        )
        .await?;
        let container_ref = created_workspace
            .workspace_dir
            .to_string_lossy()
            .to_string();
        Workspace::update_container_ref(&service.db.pool, workspace.id, &container_ref).await?;
        Ok::<_, ContainerError>(container_ref)
    }
    .await;

    let (next, reason) = match &result {
        Ok(_) => (WorkspacePlacementState::Ready, None),
        Err(error) => (WorkspacePlacementState::Failed, Some(error.to_string())),
    };
    let transitioned = WorkspacePlacement::transition(
        &service.db.pool,
        workspace.id,
        WorkspacePlacementState::Provisioning,
        next,
        reason.as_deref(),
    )
    .await?;
    if !transitioned && result.is_ok() {
        return Err(ContainerError::Other(anyhow!(
            "Workspace provisioning completed but ready state could not be persisted"
        )));
    }
    result
}

fn success_exit_status() -> std::process::ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        ExitStatusExt::from_raw(0)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt;
        ExitStatusExt::from_raw(0)
    }
}

#[cfg(test)]
mod mcp_snapshot_tests {
    use std::collections::BTreeMap;

    use executors::executors::BaseCodingAgent;
    use serde_json::json;

    use super::validated_mcp_snapshot;

    #[test]
    fn snapshot_builder_preserves_non_codex_executor_identity_and_definition() {
        for executor in [BaseCodingAgent::ClaudeCode, BaseCodingAgent::Gemini] {
            let snapshot = validated_mcp_snapshot(
                executor,
                BTreeMap::from([(
                    "settings-owned".into(),
                    json!({"type": "http", "url": "https://example.invalid/mcp"}),
                )]),
            )
            .unwrap();

            assert_eq!(snapshot.executor, executor.to_string());
            assert!(snapshot.servers.contains_key("settings-owned"));
        }
    }
}

#[cfg(test)]
mod queued_follow_up_tests {
    use super::{SkippedCleanupAction, skipped_cleanup_action};

    #[test]
    fn skipped_cleanup_dispatches_a_queued_follow_up() {
        assert_eq!(
            skipped_cleanup_action(true),
            SkippedCleanupAction::StartQueuedFollowUp
        );
    }

    #[test]
    fn skipped_cleanup_finalizes_without_a_queued_follow_up() {
        assert_eq!(
            skipped_cleanup_action(false),
            SkippedCleanupAction::Finalize
        );
    }
}

#[cfg(test)]
mod worker_event_tests {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
    use utils::{log_msg::LogMsg, msg_store::MsgStore};

    use super::push_worker_bytes;

    #[test]
    fn worker_output_is_forwarded_to_msg_store_in_received_order() {
        let store = MsgStore::new();
        push_worker_bytes(&store, &BASE64_STANDARD.encode("first"), false);
        push_worker_bytes(&store, &BASE64_STANDARD.encode("second"), true);
        push_worker_bytes(&store, &BASE64_STANDARD.encode("third"), false);

        assert!(matches!(store.get_history().as_slice(), [
            LogMsg::Stdout(first),
            LogMsg::Stderr(second),
            LogMsg::Stdout(third),
        ] if first == "first" && second == "second" && third == "third"));
    }

    #[test]
    fn invalid_worker_output_is_explicitly_reported() {
        let store = MsgStore::new();
        push_worker_bytes(&store, "not-base64", false);
        assert!(matches!(
            store.get_history().as_slice(),
            [LogMsg::Stderr(message)] if message.contains("invalid base64")
        ));
    }
}

#[cfg(test)]
mod cluster_cleanup_tests {
    use chrono::{Duration, Utc};
    use db::models::worker_node::{WorkerMountStatus, WorkerNode, WorkerNodeStatus};
    use serde_json::json;
    use uuid::Uuid;

    use super::worker_cleanup_evidence_safe;

    fn worker(status: WorkerNodeStatus, lease_offset_seconds: i64) -> WorkerNode {
        let now = Utc::now();
        WorkerNode {
            id: Uuid::new_v4(),
            hostname: "think3".into(),
            status,
            worker_version: "1".into(),
            vibe_version: "1".into(),
            capabilities: json!({}).into(),
            resource_snapshot: json!({}).into(),
            labels: json!({}).into(),
            mount_status: WorkerMountStatus::Healthy,
            mount_message: None,
            last_heartbeat_at: Some(now),
            lease_expires_at: Some(now + Duration::seconds(lease_offset_seconds)),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn cleanup_requires_current_worker_evidence_and_no_unsafe_jobs() {
        let now = Utc::now();
        assert!(worker_cleanup_evidence_safe(
            &worker(WorkerNodeStatus::Online, 30),
            false,
            now
        ));
        assert!(!worker_cleanup_evidence_safe(
            &worker(WorkerNodeStatus::Offline, 30),
            false,
            now
        ));
        assert!(!worker_cleanup_evidence_safe(
            &worker(WorkerNodeStatus::Online, -1),
            false,
            now
        ));
        assert!(!worker_cleanup_evidence_safe(
            &worker(WorkerNodeStatus::Online, 30),
            true,
            now
        ));
    }
}

#[cfg(test)]
mod warm_tests {
    use std::{sync::Arc, time::Duration};

    use command_group::{AsyncCommandGroup, AsyncGroupChild};
    use tokio::sync::RwLock;
    use uuid::Uuid;

    use super::{
        WARM_IDLE_TIMEOUT, WarmAppServer, WarmRegistry, WarmReuseHandle, is_reserved_env_name,
        parse_keep_warm, reap_all_warm_entries, reap_warm_entry, reap_warm_entry_if_unchanged,
        register_warm_entry, should_keep_warm, sweep_idle_warm_entries, take_live_warm_entry,
        warm_entry_is_idle, workspace_go_environment,
    };

    #[test]
    fn organization_environment_cannot_replace_runtime_contract() {
        for name in [
            "VK_WORKSPACE_ID",
            "VK_WORKSPACE_BRANCH",
            "GOBIN",
            "GOCACHE",
            "GOENV",
            "GOMODCACHE",
            "GOPATH",
            "GOTOOLCHAIN",
            "PATH",
            "HOME",
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
            "OPENCODE_SERVER_PASSWORD",
        ] {
            assert!(is_reserved_env_name(name), "{name} must stay reserved");
        }
        assert!(!is_reserved_env_name("GITHUB_TOKEN"));
        assert!(!is_reserved_env_name("AZURE_CLIENT_ID"));
    }

    #[test]
    fn go_environment_is_stable_and_workspace_scoped() {
        let inherited = std::ffi::OsStr::new("/host/bin:/usr/bin");
        let first =
            workspace_go_environment(std::path::Path::new("/workspaces/first"), Some(inherited));
        let first_again =
            workspace_go_environment(std::path::Path::new("/workspaces/first"), Some(inherited));
        let second =
            workspace_go_environment(std::path::Path::new("/workspaces/second"), Some(inherited));

        assert_eq!(first, first_again);
        assert_eq!(first["GOTOOLCHAIN"], "auto");
        for name in ["GOBIN", "GOCACHE", "GOENV", "GOMODCACHE", "GOPATH"] {
            assert!(first[name].starts_with("/workspaces/first/.vibe-kanban/go/"));
            assert!(second[name].starts_with("/workspaces/second/.vibe-kanban/go/"));
            assert_ne!(first[name], second[name]);
        }
        assert_eq!(
            std::env::split_paths(std::ffi::OsStr::new(&first["PATH"]))
                .next()
                .unwrap(),
            std::path::PathBuf::from(&first["GOBIN"])
        );

        let dispatched = workspace_go_environment(std::path::Path::new("/workspaces/first"), None);
        assert!(!dispatched.contains_key("PATH"));
    }

    // The exit-monitor keeps a persistent app-server warm only on a clean turn
    // end: warm executor + success + not explicitly stopped. Every other cell of
    // the matrix must fall through to the process-group kill (returns false).
    #[test]
    fn warm_success_not_stopped_is_kept_warm() {
        assert!(should_keep_warm(true, true, false));
    }

    #[test]
    fn warm_failure_is_reaped() {
        assert!(!should_keep_warm(true, false, false));
    }

    #[test]
    fn warm_success_but_stopped_is_reaped() {
        // An explicit user stop must terminate the process (FR-5), even on a
        // success-coded signal.
        assert!(!should_keep_warm(true, true, true));
    }

    #[test]
    fn non_warm_executor_is_always_reaped() {
        // One-shot CLI executors (keep_warm = false) are unaffected (FR-3):
        // never kept warm regardless of success/stopped.
        assert!(!should_keep_warm(false, true, false));
        assert!(!should_keep_warm(false, false, false));
        assert!(!should_keep_warm(false, true, true));
    }

    // ===== Phase 2 warm registry ==================================

    fn handle(url: &str) -> WarmReuseHandle {
        WarmReuseHandle {
            base_url: url.to_string(),
            server_password: "secret".to_string(),
            agent_session_id: None,
        }
    }

    /// A long-lived child process to stand in for a warm app-server.
    async fn spawn_live_child() -> Arc<RwLock<AsyncGroupChild>> {
        let child = tokio::process::Command::new("sleep")
            .arg("300")
            .group_spawn()
            .expect("spawn sleep");
        Arc::new(RwLock::new(child))
    }

    /// A child that has already exited and been reaped (stands in for a warm
    /// server that died out-of-band).
    async fn spawn_dead_child() -> Arc<RwLock<AsyncGroupChild>> {
        let arc = spawn_live_child().await;
        {
            let mut c = arc.write().await;
            let _ = c.kill().await;
            let _ = c.wait().await;
        }
        arc
    }

    fn empty_registry() -> WarmRegistry {
        RwLock::new(std::collections::HashMap::new())
    }

    #[test]
    fn gate_parsing_default_off_truthy_on() {
        // Default (unset/empty) and junk values are off; only explicit truthy
        // strings turn it on (spec FR-8).
        for off in ["", "  ", "0", "false", "no", "off", "nope"] {
            assert!(!parse_keep_warm(off), "{off:?} should be off");
        }
        for on in ["1", "true", "TRUE", " yes ", "On"] {
            assert!(parse_keep_warm(on), "{on:?} should be on");
        }
    }

    #[test]
    fn idle_predicate_uses_timeout() {
        let base = std::time::Instant::now();
        let timeout = Duration::from_secs(60);
        // Fresh: not idle. Exactly-and-past the timeout: idle.
        assert!(!warm_entry_is_idle(base, base, timeout));
        assert!(!warm_entry_is_idle(
            base,
            base + Duration::from_secs(59),
            timeout
        ));
        assert!(warm_entry_is_idle(base, base + timeout, timeout));
        assert!(warm_entry_is_idle(
            base,
            base + Duration::from_secs(120),
            timeout
        ));
    }

    #[tokio::test]
    async fn register_then_take_returns_live_child() {
        let reg = empty_registry();
        let session = Uuid::new_v4();
        register_warm_entry(&reg, session, spawn_live_child().await, handle("http://a")).await;

        let taken = take_live_warm_entry(&reg, &session).await;
        assert!(taken.is_some(), "a live warm entry is a reuse hit (FR-2)");
        let (mut child, reuse) = taken.unwrap();
        assert_eq!(reuse.base_url, "http://a");
        // The take removed it: registry is empty, no double-reuse.
        assert!(reg.read().await.is_empty());
        let _ = child.kill().await;
    }

    #[tokio::test]
    async fn register_enforces_one_per_session() {
        let reg = empty_registry();
        let session = Uuid::new_v4();
        // Registering a second warm server for the same session reaps the first
        // and keeps exactly one entry (FR-1/FR-7).
        register_warm_entry(
            &reg,
            session,
            spawn_live_child().await,
            handle("http://first"),
        )
        .await;
        register_warm_entry(
            &reg,
            session,
            spawn_live_child().await,
            handle("http://second"),
        )
        .await;
        assert_eq!(reg.read().await.len(), 1);
        let (mut child, reuse) = take_live_warm_entry(&reg, &session).await.unwrap();
        assert_eq!(reuse.base_url, "http://second", "the newer server wins");
        let _ = child.kill().await;
    }

    #[tokio::test]
    async fn reap_removes_and_kills() {
        let reg = empty_registry();
        let session = Uuid::new_v4();
        let child = spawn_live_child().await;
        let watch = child.clone(); // inspect liveness after reap
        register_warm_entry(&reg, session, child, handle("http://a")).await;

        reap_warm_entry(&reg, &session).await;
        assert!(reg.read().await.is_empty(), "reap drops the entry (FR-4)");

        // The process was killed by the reap.
        let mut guard = watch.write().await;
        let _ = guard.wait().await;
        assert!(
            matches!(guard.try_wait(), Ok(Some(_)) | Err(_)),
            "reaped process is dead"
        );
    }

    #[tokio::test]
    async fn reap_is_idempotent_noop_when_absent() {
        let reg = empty_registry();
        // Reaping a session with no warm entry is a harmless no-op (FR-4).
        reap_warm_entry(&reg, &Uuid::new_v4()).await;
        assert!(reg.read().await.is_empty());
    }

    #[tokio::test]
    async fn take_dead_entry_is_miss_and_reaped() {
        let reg = empty_registry();
        let session = Uuid::new_v4();
        register_warm_entry(
            &reg,
            session,
            spawn_dead_child().await,
            handle("http://dead"),
        )
        .await;

        // A dead warm server is not attached to — it is treated as a miss so the
        // caller cold-starts, and the entry is dropped (FR-6).
        assert!(take_live_warm_entry(&reg, &session).await.is_none());
        assert!(reg.read().await.is_empty());
    }

    #[tokio::test]
    async fn shutdown_reaps_all_warm_entries() {
        let reg = empty_registry();
        // On shutdown every parked warm server is reaped (FR-4c) — none survive.
        register_warm_entry(
            &reg,
            Uuid::new_v4(),
            spawn_live_child().await,
            handle("http://a"),
        )
        .await;
        register_warm_entry(
            &reg,
            Uuid::new_v4(),
            spawn_live_child().await,
            handle("http://b"),
        )
        .await;
        assert_eq!(reg.read().await.len(), 2);

        reap_all_warm_entries(&reg).await;
        assert!(
            reg.read().await.is_empty(),
            "all warm entries reaped on shutdown"
        );
    }

    #[tokio::test]
    async fn conditional_reap_respects_generation() {
        let reg = empty_registry();
        let session = Uuid::new_v4();
        let generation = std::time::Instant::now();
        let child = spawn_live_child().await;
        reg.write().await.insert(
            session,
            WarmAppServer {
                child,
                pgid: None,
                reuse: handle("http://x"),
                last_active: generation,
            },
        );

        // A stale (non-matching) generation must NOT reap — models a server that
        // was reaped + re-registered after the sweep inspected it.
        reap_warm_entry_if_unchanged(&reg, &session, generation + Duration::from_secs(1)).await;
        assert_eq!(
            reg.read().await.len(),
            1,
            "mismatched generation is not reaped"
        );

        // The matching generation reaps it.
        reap_warm_entry_if_unchanged(&reg, &session, generation).await;
        assert!(reg.read().await.is_empty(), "matching generation is reaped");
    }

    #[tokio::test]
    async fn idle_sweep_reaps_stale_entry() {
        let reg = empty_registry();
        let session = Uuid::new_v4();
        // Insert directly with a last_active well past the timeout to exercise the
        // idle branch deterministically (FR-5).
        let child = spawn_live_child().await;
        let watch = child.clone();
        let stale_since = std::time::Instant::now()
            .checked_sub(WARM_IDLE_TIMEOUT + Duration::from_secs(60))
            .unwrap_or_else(std::time::Instant::now);
        reg.write().await.insert(
            session,
            WarmAppServer {
                child,
                pgid: None,
                reuse: handle("http://idle"),
                last_active: stale_since,
            },
        );

        sweep_idle_warm_entries(&reg).await;
        assert!(reg.read().await.is_empty(), "idle entry reaped (FR-5)");
        let mut guard = watch.write().await;
        let _ = guard.wait().await;
        assert!(matches!(guard.try_wait(), Ok(Some(_)) | Err(_)));
    }

    #[tokio::test]
    async fn idle_sweep_keeps_fresh_live_entry_but_reaps_dead() {
        let reg = empty_registry();
        let fresh = Uuid::new_v4();
        let dead = Uuid::new_v4();
        register_warm_entry(
            &reg,
            fresh,
            spawn_live_child().await,
            handle("http://fresh"),
        )
        .await;
        register_warm_entry(&reg, dead, spawn_dead_child().await, handle("http://dead")).await;

        sweep_idle_warm_entries(&reg).await;

        let map = reg.read().await;
        assert!(
            map.contains_key(&fresh),
            "a fresh live entry survives the sweep"
        );
        assert!(
            !map.contains_key(&dead),
            "a dead entry is reaped by the sweep (FR-6)"
        );
        drop(map);
        // cleanup
        if let Some((mut child, _)) = take_live_warm_entry(&reg, &fresh).await {
            let _ = child.kill().await;
        }
    }
}
