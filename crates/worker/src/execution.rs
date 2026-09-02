use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::{Duration, SystemTime},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use chrono::Utc;
use cluster_protocol::{
    DispatchAccepted, EventBatch, ExecutionDispatch, ExecutionEventPayload, JobState, JobSummary,
    McpConfigSnapshot, TerminalEvidence, TerminalState, WorkerMcpRefreshResult,
    WorkerMcpRefreshStatus,
};
use command_group::{AsyncCommandGroup, AsyncGroupChild};
use executors::{
    actions::{Executable, ExecutorAction, ExecutorActionType},
    env::{ExecutionEnv, RepoContext},
    executors::{BaseCodingAgent, CodingAgent, StandardCodingAgentExecutor},
    mcp_config::write_coding_agent_mcp_servers_to_path,
    mcp_refresh::McpRefreshHandle,
    profile::{ExecutorConfig, ExecutorProfile},
};
use serde::Deserialize;
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    sync::{Mutex, RwLock},
};
use utils::worktree_linkage::{LinkageStatus, WorktreeLinkage};
use uuid::Uuid;

use crate::{
    interaction::{InteractionBroker, WorkerApprovalService},
    journal::{EventJournal, JournalError},
    path_authority::{PathAuthority, PathAuthorityError},
    recovery::{RecoveryError, RecoveryStore},
};

const DEFAULT_JOURNAL_CAPACITY: usize = 4_096;

#[derive(Debug, Deserialize)]
struct WorkerCommandAction {
    program: String,
    #[serde(default)]
    args: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WorkerAction {
    Command(WorkerCommandAction),
    Executor(ExecutorAction),
}

fn executor_config_for_action(action: &WorkerAction) -> Option<&ExecutorConfig> {
    let WorkerAction::Executor(action) = action else {
        return None;
    };
    match action.typ() {
        ExecutorActionType::CodingAgentInitialRequest(request) => Some(&request.executor_config),
        ExecutorActionType::CodingAgentFollowUpRequest(request) => Some(&request.executor_config),
        ExecutorActionType::ReviewRequest(request) => Some(&request.executor_config),
        ExecutorActionType::ScriptRequest(_) => None,
    }
}

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("execution {execution_id} was already dispatched with a different request digest")]
    DigestConflict { execution_id: Uuid },
    #[error("execution action is invalid: {0}")]
    InvalidAction(#[from] serde_json::Error),
    #[error("execution action program must not be empty")]
    EmptyProgram,
    #[error("MCP configuration snapshot is invalid for executor {executor}")]
    InvalidMcpSnapshot { executor: String },
    #[error("failed to materialize MCP configuration for executor {executor}")]
    McpMaterialization { executor: String },
    #[error("Codex MCP status is unavailable")]
    McpReload,
    #[error("working directory resolves outside its authorized workspace")]
    WorkingDirectoryOutsideWorkspace,
    #[error(transparent)]
    PathAuthority(#[from] PathAuthorityError),
    #[error(transparent)]
    Journal(#[from] JournalError),
    #[error("execution {0} was not found")]
    NotFound(Uuid),
    #[error("worker is draining for a release handoff")]
    Draining,
    #[error("workspace contains a worktree this node cannot use: {detail}")]
    WorktreeUnusable { detail: String },
    #[error(transparent)]
    Recovery(#[from] RecoveryError),
}

#[derive(Clone)]
pub struct ExecutionSupervisor {
    path_authority: PathAuthority,
    mcp_config_root: PathBuf,
    coordinator_url: reqwest::Url,
    jobs: Arc<RwLock<HashMap<Uuid, Arc<WorkerJob>>>>,
    journal_capacity: usize,
    recovery_store: Option<RecoveryStore>,
    admission_draining: Arc<AtomicBool>,
    admitting: Arc<AtomicU32>,
}

pub struct WorkerJob {
    execution_id: Uuid,
    worker_job_id: Uuid,
    workspace_id: Uuid,
    request_digest: String,
    state: RwLock<JobState>,
    journal: Mutex<EventJournal>,
    child: Mutex<Option<AsyncGroupChild>>,
    cancellation: Mutex<()>,
    acknowledged_sequence: Mutex<u64>,
    recovery_store: Option<RecoveryStore>,
    interactions: Arc<InteractionBroker>,
    mcp_config: Mutex<Option<PreparedMcpConfig>>,
    mcp_refresh: RwLock<Option<McpRefreshHandle>>,
    mcp_refresh_claim: Mutex<()>,
    quiesced_by: Mutex<Option<Uuid>>,
}

struct PreparedMcpConfig {
    execution_root: PathBuf,
    target_config: PathBuf,
    environment: std::collections::BTreeMap<String, String>,
    agent: CodingAgent,
}

impl Drop for PreparedMcpConfig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.execution_root);
    }
}

fn runtime_mcp_servers(
    snapshot: &McpConfigSnapshot,
    coordinator_url: &reqwest::Url,
) -> HashMap<String, serde_json::Value> {
    snapshot
        .servers
        .iter()
        .map(|(name, entry)| {
            let mut entry = entry.clone();
            if let Some(url) = entry.get("url").and_then(serde_json::Value::as_str)
                && let Some(runtime_url) = runtime_gateway_url(url, coordinator_url)
            {
                entry["url"] = serde_json::Value::String(runtime_url);
            }
            (name.clone(), entry)
        })
        .collect()
}

fn runtime_gateway_url(configured: &str, coordinator_url: &reqwest::Url) -> Option<String> {
    let configured = reqwest::Url::parse(configured).ok()?;
    if !matches!(configured.scheme(), "http" | "https")
        || !configured.host_str().is_some_and(|host| {
            host.eq_ignore_ascii_case("localhost")
                || host
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|address| address.is_loopback())
        })
        || !configured.path().starts_with("/mcp-gateway/")
    {
        return None;
    }

    let mut runtime = coordinator_url.clone();
    let base_path = runtime.path().trim_end_matches('/').to_owned();
    runtime.set_path(&format!("{base_path}{}", configured.path()));
    runtime.set_query(configured.query());
    runtime.set_fragment(configured.fragment());
    Some(runtime.into())
}

fn prepare_scoped_home(
    source_home: &Path,
    scoped_home: &Path,
    target_relative: &Path,
) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fn overlay_directory(
        source: Option<&Path>,
        scoped: &Path,
        target: &Path,
    ) -> std::io::Result<()> {
        use std::os::unix::fs::symlink;

        std::fs::create_dir_all(scoped)?;
        let mut components = target.components();
        let target_name = components.next().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "empty config path")
        })?;
        let remainder = components.as_path();
        if let Some(source) = source {
            let backup_name = target_name
                .as_os_str()
                .to_str()
                .map(|name| format!("{name}.bak"));
            for entry in std::fs::read_dir(source)? {
                let entry = entry?;
                if entry.file_name() == target_name.as_os_str()
                    || backup_name
                        .as_deref()
                        .is_some_and(|name| entry.file_name() == name)
                {
                    continue;
                }
                symlink(entry.path(), scoped.join(entry.file_name()))?;
            }
        }
        if !remainder.as_os_str().is_empty() {
            let source_child = source.map(|source| source.join(target_name.as_os_str()));
            let source_child = source_child.as_deref().filter(|path| path.is_dir());
            overlay_directory(
                source_child,
                &scoped.join(target_name.as_os_str()),
                remainder,
            )?;
        }
        Ok(())
    }

    if target_relative.is_absolute()
        || target_relative
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "config path escapes scoped home",
        ));
    }
    let source_home = match source_home.canonicalize() {
        Ok(source_home) => Some(source_home),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    std::fs::create_dir_all(scoped_home)?;
    std::fs::set_permissions(scoped_home, std::fs::Permissions::from_mode(0o700))?;
    overlay_directory(source_home.as_deref(), scoped_home, target_relative)?;
    Ok(())
}

fn non_codex_scoped_config_layout(
    source_config: &Path,
    source_home: PathBuf,
    source_xdg: Option<PathBuf>,
    execution_root: &Path,
) -> Option<(
    PathBuf,
    PathBuf,
    PathBuf,
    std::collections::BTreeMap<String, String>,
)> {
    if let Ok(target_relative) = source_config.strip_prefix(&source_home) {
        let target_relative = target_relative.to_path_buf();
        let scoped_home = execution_root.join("home");
        let mut environment = std::collections::BTreeMap::from([(
            "HOME".into(),
            scoped_home.to_string_lossy().into_owned(),
        )]);
        if target_relative.starts_with(".config") {
            environment.insert(
                "XDG_CONFIG_HOME".into(),
                scoped_home.join(".config").to_string_lossy().into_owned(),
            );
        }
        return Some((source_home, scoped_home, target_relative, environment));
    }

    let source_xdg = source_xdg?;
    let target_relative = source_config.strip_prefix(&source_xdg).ok()?.to_path_buf();
    let scoped_xdg = execution_root.join("xdg");
    let environment = std::collections::BTreeMap::from([(
        "XDG_CONFIG_HOME".into(),
        scoped_xdg.to_string_lossy().into_owned(),
    )]);
    Some((source_xdg, scoped_xdg, target_relative, environment))
}

struct ExecutionAdmission(Arc<AtomicU32>);

impl Drop for ExecutionAdmission {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

impl ExecutionSupervisor {
    pub async fn authorizes_session_transfer(
        &self,
        execution_id: Uuid,
        workspace_id: Uuid,
    ) -> bool {
        let Some(job) = self.job(execution_id).await else {
            return false;
        };
        job.workspace_id == workspace_id && job.state().await == JobState::Running
    }

    #[cfg(test)]
    pub fn new(path_authority: PathAuthority) -> Self {
        Self::with_journal_capacity(path_authority, DEFAULT_JOURNAL_CAPACITY)
    }

    #[cfg(test)]
    pub fn with_journal_capacity(path_authority: PathAuthority, journal_capacity: usize) -> Self {
        let mcp_config_root = std::env::temp_dir().join("vibe-kanban-worker-tests/mcp-config");
        Self {
            path_authority,
            mcp_config_root,
            coordinator_url: reqwest::Url::parse("http://127.0.0.1:3334").unwrap(),
            jobs: Arc::new(RwLock::new(HashMap::new())),
            journal_capacity,
            recovery_store: None,
            admission_draining: Arc::new(AtomicBool::new(false)),
            admitting: Arc::new(AtomicU32::new(0)),
        }
    }

    #[cfg(test)]
    fn with_runtime_config(
        path_authority: PathAuthority,
        mcp_config_root: PathBuf,
        coordinator_url: reqwest::Url,
    ) -> Self {
        let mut supervisor = Self::new(path_authority);
        supervisor.mcp_config_root = mcp_config_root;
        supervisor.coordinator_url = coordinator_url;
        supervisor
    }

    pub async fn with_recovery(
        path_authority: PathAuthority,
        recovery_store: RecoveryStore,
        mcp_config_root: PathBuf,
        coordinator_url: reqwest::Url,
    ) -> Result<Self, ExecutionError> {
        Self::with_recovery_and_drain(
            path_authority,
            recovery_store,
            Arc::new(AtomicBool::new(false)),
            mcp_config_root,
            coordinator_url,
        )
        .await
    }

    pub async fn with_recovery_and_drain(
        path_authority: PathAuthority,
        recovery_store: RecoveryStore,
        admission_draining: Arc<AtomicBool>,
        mcp_config_root: PathBuf,
        coordinator_url: reqwest::Url,
    ) -> Result<Self, ExecutionError> {
        let supervisor = Self {
            path_authority,
            mcp_config_root,
            coordinator_url,
            jobs: Arc::new(RwLock::new(HashMap::new())),
            journal_capacity: DEFAULT_JOURNAL_CAPACITY,
            recovery_store: Some(recovery_store.clone()),
            admission_draining,
            admitting: Arc::new(AtomicU32::new(0)),
        };
        for mut summary in recovery_store.load().await? {
            let evidence = match summary.terminal.clone() {
                Some(evidence) if summary.state.is_terminal() => evidence,
                _ => {
                    let evidence = TerminalEvidence {
                        state: TerminalState::Interrupted,
                        exit_code: None,
                        signal: None,
                        observed_at: Utc::now(),
                    };
                    summary.state = JobState::Interrupted;
                    summary.terminal = Some(evidence.clone());
                    recovery_store.save(&summary).await?;
                    evidence
                }
            };
            let journal = EventJournal::recover(&summary, DEFAULT_JOURNAL_CAPACITY, evidence)?;
            summary.last_sequence = journal.last_sequence();
            recovery_store.save(&summary).await?;
            supervisor.jobs.write().await.insert(
                summary.execution_id,
                Arc::new(WorkerJob {
                    execution_id: summary.execution_id,
                    worker_job_id: summary.worker_job_id,
                    workspace_id: summary.workspace_id,
                    request_digest: summary.request_digest,
                    state: RwLock::new(summary.state),
                    journal: Mutex::new(journal),
                    child: Mutex::new(None),
                    cancellation: Mutex::new(()),
                    acknowledged_sequence: Mutex::new(0),
                    recovery_store: Some(recovery_store.clone()),
                    interactions: Arc::new(InteractionBroker::default()),
                    mcp_config: Mutex::new(None),
                    mcp_refresh: RwLock::new(None),
                    mcp_refresh_claim: Mutex::new(()),
                    quiesced_by: Mutex::new(None),
                }),
            );
        }
        Ok(supervisor)
    }

    /// Accepts a dispatch exactly once per execution ID. Replays with the same
    /// digest return the existing job; a different digest is never started.
    pub async fn dispatch(
        &self,
        dispatch: ExecutionDispatch,
    ) -> Result<DispatchAccepted, ExecutionError> {
        let mut jobs = self.jobs.write().await;
        if let Some(existing) = jobs.get(&dispatch.execution_id) {
            if existing.request_digest != dispatch.request_digest {
                return Err(ExecutionError::DigestConflict {
                    execution_id: dispatch.execution_id,
                });
            }
            return Ok(existing.accepted().await);
        }
        if self.admission_draining.load(Ordering::Acquire) {
            return Err(ExecutionError::Draining);
        }
        self.admitting.fetch_add(1, Ordering::AcqRel);
        let admission = ExecutionAdmission(self.admitting.clone());
        if self.admission_draining.load(Ordering::Acquire) {
            return Err(ExecutionError::Draining);
        }

        let workspace_path = self
            .path_authority
            .authorize_workspace_path(&dispatch.workspace_path)?;
        let working_directory = authorize_working_directory(
            &workspace_path,
            &dispatch.working_directory,
            &self.path_authority,
        )?;
        let action: WorkerAction = serde_json::from_value(dispatch.action.clone())?;
        // Refuse at admission rather than inside the turn: a malformed profile
        // would otherwise fail at spawn, after the job record exists and the
        // user is waiting on an agent that was never going to start.
        let executor_profile: Option<ExecutorProfile> = dispatch
            .executor_profile_config
            .clone()
            .map(serde_json::from_value)
            .transpose()?;
        let prepared_mcp = match &dispatch.mcp_config_snapshot {
            Some(snapshot) => Some(
                self.prepare_mcp_snapshot(
                    dispatch.execution_id,
                    &action,
                    executor_profile.as_ref(),
                    snapshot,
                )
                .await?,
            ),
            None => None,
        };
        if matches!(&action, WorkerAction::Command(action) if action.program.trim().is_empty()) {
            return Err(ExecutionError::EmptyProgram);
        }
        // Refuse before a job record exists, alongside the other admission
        // checks. A workspace whose worktrees do not resolve on this node cannot
        // be worked in, and discovering that inside an agent turn wastes the
        // turn. The worker never repairs — that authority is the coordinator's.
        verify_worktrees_are_usable(&workspace_path, self.path_authority.shared_root()).await?;

        let worker_job_id = Uuid::new_v4();
        let mut journal = EventJournal::new(dispatch.execution_id, self.journal_capacity)?;
        journal.append(SystemTime::now(), ExecutionEventPayload::Accepted)?;
        let job = Arc::new(WorkerJob {
            execution_id: dispatch.execution_id,
            worker_job_id,
            workspace_id: dispatch.workspace_id,
            request_digest: dispatch.request_digest.clone(),
            state: RwLock::new(JobState::Accepted),
            journal: Mutex::new(journal),
            child: Mutex::new(None),
            cancellation: Mutex::new(()),
            acknowledged_sequence: Mutex::new(0),
            recovery_store: self.recovery_store.clone(),
            interactions: Arc::new(InteractionBroker::default()),
            mcp_config: Mutex::new(None),
            mcp_refresh: RwLock::new(None),
            mcp_refresh_claim: Mutex::new(()),
            quiesced_by: Mutex::new(None),
        });
        jobs.insert(dispatch.execution_id, job.clone());
        drop(admission);
        drop(jobs);
        job.persist().await;

        tokio::spawn(run_job(
            job.clone(),
            action,
            working_directory,
            dispatch.environment,
            executor_profile,
            prepared_mcp,
            dispatch.timeout_seconds,
        ));
        Ok(DispatchAccepted {
            execution_id: dispatch.execution_id,
            worker_job_id,
            request_digest: dispatch.request_digest,
            state: JobState::Accepted,
            last_sequence: 1,
        })
    }

    async fn prepare_mcp_snapshot(
        &self,
        execution_id: Uuid,
        action: &WorkerAction,
        profile: Option<&ExecutorProfile>,
        snapshot: &McpConfigSnapshot,
    ) -> Result<PreparedMcpConfig, ExecutionError> {
        snapshot
            .validate_size()
            .map_err(|_| ExecutionError::InvalidMcpSnapshot {
                executor: snapshot.executor.clone(),
            })?;
        let config = executor_config_for_action(action).ok_or_else(|| {
            ExecutionError::InvalidMcpSnapshot {
                executor: snapshot.executor.clone(),
            }
        })?;
        if snapshot.executor != config.executor.to_string() {
            return Err(ExecutionError::InvalidMcpSnapshot {
                executor: snapshot.executor.clone(),
            });
        }
        let variant = config.variant.as_deref().unwrap_or("DEFAULT");
        let agent = profile
            .and_then(|profile| profile.get_variant(variant))
            .ok_or_else(|| ExecutionError::InvalidMcpSnapshot {
                executor: snapshot.executor.clone(),
            })?;
        let source_config =
            agent
                .default_mcp_config_path()
                .ok_or_else(|| ExecutionError::InvalidMcpSnapshot {
                    executor: snapshot.executor.clone(),
                })?;
        let execution_root = self.mcp_config_root.join(execution_id.to_string());
        let (source_home, scoped_home, target_relative, environment) =
            if config.executor == BaseCodingAgent::Codex {
                let source_home =
                    source_config
                        .parent()
                        .ok_or_else(|| ExecutionError::InvalidMcpSnapshot {
                            executor: snapshot.executor.clone(),
                        })?;
                let scoped_home = execution_root.join("codex");
                let target_relative = PathBuf::from("config.toml");
                let environment = std::collections::BTreeMap::from([(
                    "CODEX_HOME".into(),
                    scoped_home.to_string_lossy().into_owned(),
                )]);
                (
                    source_home.to_path_buf(),
                    scoped_home,
                    target_relative,
                    environment,
                )
            } else {
                let source_home = std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| {
                    ExecutionError::InvalidMcpSnapshot {
                        executor: snapshot.executor.clone(),
                    }
                })?;
                non_codex_scoped_config_layout(
                    &source_config,
                    source_home,
                    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
                    &execution_root,
                )
                .ok_or_else(|| ExecutionError::InvalidMcpSnapshot {
                    executor: snapshot.executor.clone(),
                })?
            };
        match std::fs::remove_dir_all(&execution_root) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {
                return Err(ExecutionError::McpMaterialization {
                    executor: snapshot.executor.clone(),
                });
            }
        }
        prepare_scoped_home(&source_home, &scoped_home, &target_relative).map_err(|_| {
            ExecutionError::McpMaterialization {
                executor: snapshot.executor.clone(),
            }
        })?;
        let target_config = scoped_home.join(&target_relative);
        let prepared = PreparedMcpConfig {
            execution_root,
            target_config,
            environment,
            agent: agent.clone(),
        };
        let servers = runtime_mcp_servers(snapshot, &self.coordinator_url);
        write_coding_agent_mcp_servers_to_path(
            agent,
            &source_config,
            &prepared.target_config,
            &servers,
        )
        .await
        .map_err(|_| ExecutionError::McpMaterialization {
            executor: snapshot.executor.clone(),
        })?;
        Ok(prepared)
    }

    pub async fn refresh_mcp(
        &self,
        execution_id: Uuid,
        snapshot: &McpConfigSnapshot,
    ) -> Result<WorkerMcpRefreshResult, ExecutionError> {
        snapshot
            .validate_size()
            .map_err(|_| ExecutionError::InvalidMcpSnapshot {
                executor: snapshot.executor.clone(),
            })?;
        let job = self
            .jobs
            .read()
            .await
            .get(&execution_id)
            .cloned()
            .ok_or(ExecutionError::NotFound(execution_id))?;
        let Ok(_claim) = job.mcp_refresh_claim.try_lock() else {
            return Ok(WorkerMcpRefreshResult {
                status: WorkerMcpRefreshStatus::Busy,
                servers: Vec::new(),
            });
        };
        if job.state().await.is_terminal() {
            return Ok(WorkerMcpRefreshResult {
                status: WorkerMcpRefreshStatus::Unsupported,
                servers: Vec::new(),
            });
        }
        if snapshot.executor != BaseCodingAgent::Codex.to_string() {
            return Err(ExecutionError::InvalidMcpSnapshot {
                executor: snapshot.executor.clone(),
            });
        }
        let (agent, target_config) = {
            let prepared = job.mcp_config.lock().await;
            let Some(prepared) = prepared.as_ref() else {
                return Ok(WorkerMcpRefreshResult {
                    status: WorkerMcpRefreshStatus::Unsupported,
                    servers: Vec::new(),
                });
            };
            (prepared.agent.clone(), prepared.target_config.clone())
        };
        let servers = runtime_mcp_servers(snapshot, &self.coordinator_url);
        if write_coding_agent_mcp_servers_to_path(&agent, &target_config, &target_config, &servers)
            .await
            .is_err()
        {
            return Ok(WorkerMcpRefreshResult {
                status: WorkerMcpRefreshStatus::MaterializationFailed,
                servers: Vec::new(),
            });
        }
        let Some(control) = job.mcp_refresh.read().await.clone() else {
            return Ok(WorkerMcpRefreshResult {
                status: WorkerMcpRefreshStatus::ReloadFailed,
                servers: Vec::new(),
            });
        };
        if control.0.queue_refresh().await.is_err() {
            return Ok(WorkerMcpRefreshResult {
                status: WorkerMcpRefreshStatus::ReloadFailed,
                servers: Vec::new(),
            });
        }
        Ok(WorkerMcpRefreshResult {
            status: WorkerMcpRefreshStatus::Queued,
            servers: Vec::new(),
        })
    }

    pub async fn mcp_status(
        &self,
        execution_id: Uuid,
    ) -> Result<WorkerMcpRefreshResult, ExecutionError> {
        let job = self
            .jobs
            .read()
            .await
            .get(&execution_id)
            .cloned()
            .ok_or(ExecutionError::NotFound(execution_id))?;
        let Some(control) = job.mcp_refresh.read().await.clone() else {
            return Ok(WorkerMcpRefreshResult {
                status: WorkerMcpRefreshStatus::Unsupported,
                servers: Vec::new(),
            });
        };
        let servers = control
            .0
            .list_servers()
            .await
            .map_err(|_| ExecutionError::McpReload)?
            .into_iter()
            .filter_map(|server| serde_json::to_value(server).ok())
            .collect();
        Ok(WorkerMcpRefreshResult {
            status: WorkerMcpRefreshStatus::Queued,
            servers,
        })
    }

    #[cfg(unix)]
    pub async fn set_quiesced(
        &self,
        execution_id: Uuid,
        workspace_id: Uuid,
        operation_id: Uuid,
        quiesced: bool,
    ) -> Result<(), ExecutionError> {
        let job = self
            .job(execution_id)
            .await
            .ok_or(ExecutionError::NotFound(execution_id))?;
        let state = job.state().await;
        if job.workspace_id != workspace_id
            || (quiesced && state != JobState::Running)
            || (!quiesced && state.is_terminal())
        {
            return Err(ExecutionError::NotFound(execution_id));
        }
        let mut owner = job.quiesced_by.lock().await;
        if quiesced {
            if owner
                .as_ref()
                .is_some_and(|existing| *existing != operation_id)
            {
                return Err(ExecutionError::DigestConflict { execution_id });
            }
            if owner.as_ref() == Some(&operation_id) {
                return Ok(());
            }
        } else if owner.as_ref() != Some(&operation_id) {
            return Err(ExecutionError::DigestConflict { execution_id });
        }
        let pid = {
            let mut child = job.child.lock().await;
            child.as_mut().and_then(|child| child.inner().id())
        }
        .ok_or(ExecutionError::NotFound(execution_id))?;
        let signal = if quiesced { "-STOP" } else { "-CONT" };
        let target = format!("-{pid}");
        let status = tokio::process::Command::new("kill")
            .args([signal, "--", &target])
            .status()
            .await
            .map_err(|_| ExecutionError::NotFound(execution_id))?;
        if !status.success() {
            return Err(ExecutionError::NotFound(execution_id));
        }
        *owner = quiesced.then_some(operation_id);
        drop(owner);
        if quiesced {
            // A coordinator can disappear after SIGSTOP and before its
            // compensating resume. The lease is deliberately longer than the
            // coordinator's stale-operation window, giving a retry time to
            // finish while ensuring an abandoned execution is not frozen
            // forever.
            let supervisor = self.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(15 * 60)).await;
                let _ = supervisor
                    .resume_quiesced(execution_id, workspace_id, operation_id)
                    .await;
            });
        }
        Ok(())
    }

    #[cfg(unix)]
    async fn resume_quiesced(
        &self,
        execution_id: Uuid,
        workspace_id: Uuid,
        operation_id: Uuid,
    ) -> Result<(), ExecutionError> {
        let job = self
            .job(execution_id)
            .await
            .ok_or(ExecutionError::NotFound(execution_id))?;
        if job.workspace_id != workspace_id || job.state().await.is_terminal() {
            return Err(ExecutionError::NotFound(execution_id));
        }
        let mut owner = job.quiesced_by.lock().await;
        if owner.as_ref() != Some(&operation_id) {
            return Err(ExecutionError::DigestConflict { execution_id });
        }
        let pid = {
            let mut child = job.child.lock().await;
            child.as_mut().and_then(|child| child.inner().id())
        }
        .ok_or(ExecutionError::NotFound(execution_id))?;
        let target = format!("-{pid}");
        let status = tokio::process::Command::new("kill")
            .args(["-CONT", "--", &target])
            .status()
            .await
            .map_err(|_| ExecutionError::NotFound(execution_id))?;
        if !status.success() {
            return Err(ExecutionError::NotFound(execution_id));
        }
        *owner = None;
        Ok(())
    }

    pub async fn events(
        &self,
        execution_id: Uuid,
        after: u64,
    ) -> Result<EventBatch, ExecutionError> {
        let job = self
            .jobs
            .read()
            .await
            .get(&execution_id)
            .cloned()
            .ok_or(ExecutionError::NotFound(execution_id))?;
        let batch = job.journal.lock().await.replay_after(after);
        Ok(batch)
    }

    pub async fn inventory(&self) -> Vec<JobSummary> {
        let jobs = self.jobs.read().await.values().cloned().collect::<Vec<_>>();
        let mut summaries = Vec::with_capacity(jobs.len());
        for job in jobs {
            summaries.push(job.summary().await);
        }
        summaries.sort_by_key(|summary| summary.execution_id);
        summaries
    }

    pub async fn active_execution_count(&self) -> u32 {
        let jobs = self.jobs.read().await.values().cloned().collect::<Vec<_>>();
        let mut active = 0_u32;
        for job in jobs {
            if job.is_active_for_drain().await {
                active = active.saturating_add(1);
            }
        }
        active.saturating_add(self.admitting.load(Ordering::Acquire))
    }

    pub async fn acknowledge(
        &self,
        execution_id: Uuid,
        highest_contiguous_sequence: u64,
    ) -> Result<u64, ExecutionError> {
        let job = self
            .job(execution_id)
            .await
            .ok_or(ExecutionError::NotFound(execution_id))?;
        let last = job.journal.lock().await.last_sequence();
        let mut acknowledged = job.acknowledged_sequence.lock().await;
        *acknowledged = (*acknowledged).max(highest_contiguous_sequence.min(last));
        Ok(*acknowledged)
    }

    pub async fn job(&self, execution_id: Uuid) -> Option<Arc<WorkerJob>> {
        self.jobs.read().await.get(&execution_id).cloned()
    }

    pub async fn authorizes_preview(
        &self,
        execution_id: Uuid,
        workspace_id: Uuid,
        worker_job_id: Uuid,
    ) -> bool {
        let Some(job) = self.job(execution_id).await else {
            return false;
        };
        job.workspace_id == workspace_id
            && job.worker_job_id == worker_job_id
            && !job.state().await.is_terminal()
    }

    pub async fn quarantine(&self, execution_id: Uuid) -> Result<JobSummary, ExecutionError> {
        let job = self
            .job(execution_id)
            .await
            .ok_or(ExecutionError::NotFound(execution_id))?;
        *job.state.write().await = JobState::Quarantined;
        job.persist().await;
        Ok(job.summary().await)
    }

    pub async fn respond_interaction(
        &self,
        execution_id: Uuid,
        interaction_id: Uuid,
        outcome: utils::approvals::ApprovalOutcome,
    ) -> Result<bool, ExecutionError> {
        let job = self
            .job(execution_id)
            .await
            .ok_or(ExecutionError::NotFound(execution_id))?;
        let responded = job.interactions.respond(interaction_id, outcome).await;
        if responded {
            job.emit(ExecutionEventPayload::InteractionAcknowledged { interaction_id })
                .await;
        }
        Ok(responded)
    }
}

impl WorkerJob {
    async fn is_active_for_drain(&self) -> bool {
        if !self.state.read().await.is_terminal() {
            return true;
        }

        // Quarantine is a terminal protocol state, but reconciliation may put a
        // still-running process there without cancelling it. Fail closed when
        // process liveness cannot be observed so release activation never kills
        // work merely because its coordinator record was quarantined.
        let mut child = self.child.lock().await;
        match child.as_mut() {
            Some(child) => !matches!(child.try_wait(), Ok(Some(_))),
            None => false,
        }
    }

    async fn accepted(&self) -> DispatchAccepted {
        DispatchAccepted {
            execution_id: self.execution_id,
            worker_job_id: self.worker_job_id,
            request_digest: self.request_digest.clone(),
            state: self.state.read().await.clone(),
            last_sequence: self.journal.lock().await.last_sequence(),
        }
    }

    async fn summary(&self) -> JobSummary {
        let journal = self.journal.lock().await;
        JobSummary {
            execution_id: self.execution_id,
            worker_job_id: self.worker_job_id,
            workspace_id: self.workspace_id,
            request_digest: self.request_digest.clone(),
            state: self.state.read().await.clone(),
            last_sequence: journal.last_sequence(),
            terminal: journal.terminal_evidence().cloned(),
        }
    }

    async fn persist(&self) {
        if let Some(store) = &self.recovery_store
            && let Err(error) = store.save(&self.summary().await).await
        {
            tracing::error!(execution_id = %self.execution_id, "Failed to persist worker job: {error}");
        }
    }

    pub async fn child(&self) -> tokio::sync::MutexGuard<'_, Option<AsyncGroupChild>> {
        self.child.lock().await
    }

    pub(crate) async fn cancellation_guard(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.cancellation.lock().await
    }

    pub(crate) async fn state(&self) -> JobState {
        self.state.read().await.clone()
    }

    pub(crate) async fn terminal_evidence(&self) -> Option<TerminalEvidence> {
        self.journal.lock().await.terminal_evidence().cloned()
    }

    pub(crate) async fn transition(&self, state: JobState, payload: ExecutionEventPayload) {
        let appended = self
            .journal
            .lock()
            .await
            .append(SystemTime::now(), payload)
            .is_ok();
        if appended {
            *self.state.write().await = state;
            self.persist().await;
        }
    }

    pub(crate) async fn emit(&self, payload: ExecutionEventPayload) {
        let _ = self.journal.lock().await.append(SystemTime::now(), payload);
    }
}

async fn run_job(
    job: Arc<WorkerJob>,
    action: WorkerAction,
    working_directory: PathBuf,
    mut environment: std::collections::BTreeMap<String, String>,
    executor_profile: Option<ExecutorProfile>,
    prepared_mcp: Option<PreparedMcpConfig>,
    timeout_seconds: Option<u64>,
) {
    set_state(&job, JobState::Starting, ExecutionEventPayload::Starting).await;
    prepend_workspace_gobin_to_path(&mut environment);
    let inherited_path = environment
        .get("PATH")
        .map(std::ffi::OsString::from)
        .unwrap_or_else(|| std::env::var_os("PATH").unwrap_or_default());
    if let Some(path) = utils::shell::append_cli_tools_to_path(&inherited_path) {
        environment.insert("PATH".into(), path.to_string_lossy().into_owned());
    }
    if let Some(prepared) = &prepared_mcp {
        environment.extend(prepared.environment.clone());
    }
    *job.mcp_config.lock().await = prepared_mcp;
    let spawned = match action {
        WorkerAction::Command(action) => {
            let mut command = Command::new(action.program);
            command
                .args(action.args)
                .current_dir(&working_directory)
                .envs(&environment)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            command
                .group_spawn()
                .map(|child| (child, None))
                .map_err(|error| error.to_string())
        }
        WorkerAction::Executor(action) => {
            // Admission already proved this workspace is enumerable and its
            // worktrees usable, so an error here is a race, not a state worth
            // failing the spawn over.
            let repo_names = discover_repo_names(&working_directory)
                .await
                .unwrap_or_default();
            let mut env = ExecutionEnv::new(
                RepoContext::new(working_directory.clone(), repo_names),
                false,
                String::new(),
            )
            // The worker holds only the embedded default profiles, so a
            // user-defined variant is resolvable here only because the
            // coordinator sent its definition.
            .with_executor_profile(executor_profile);
            env.vars.extend(environment);
            action
                .spawn(
                    &working_directory,
                    WorkerApprovalService::new(job.clone(), job.interactions.clone()),
                    &env,
                )
                .await
                .map(|mut spawned| (spawned.child, spawned.mcp_refresh.take()))
                .map_err(|error| error.to_string())
        }
    };
    let (mut child, mcp_refresh) = match spawned {
        Ok(spawned) => spawned,
        Err(error) => {
            finish_failed(&job, None, format!("failed to start process: {error}")).await;
            return;
        }
    };
    let stdout = child.inner().stdout.take();
    let stderr = child.inner().stderr.take();
    *job.child.lock().await = Some(child);
    *job.state.write().await = JobState::Running;
    job.persist().await;

    if let Some(signal) = mcp_refresh {
        let job = job.clone();
        tokio::spawn(async move {
            if let Ok(handle) = signal.await {
                *job.mcp_refresh.write().await = Some(handle);
            }
        });
    }

    let mut stdout_task =
        stdout.map(|stdout| tokio::spawn(stream_output(job.clone(), stdout, false)));
    let mut stderr_task =
        stderr.map(|stderr| tokio::spawn(stream_output(job.clone(), stderr, true)));

    let deadline =
        timeout_seconds.map(|seconds| tokio::time::Instant::now() + Duration::from_secs(seconds));
    loop {
        let status = {
            let mut child = job.child.lock().await;
            match child
                .as_mut()
                .expect("running job must own child")
                .try_wait()
            {
                Ok(status) => status,
                Err(error) => {
                    finish_failed(&job, None, format!("failed to observe process: {error}")).await;
                    return;
                }
            }
        };
        if let Some(status) = status {
            await_output_tasks(stdout_task.take(), stderr_task.take()).await;
            finish_status(&job, status).await;
            return;
        }
        if deadline.is_some_and(|deadline| tokio::time::Instant::now() >= deadline) {
            if let Some(child) = job.child.lock().await.as_mut() {
                let _ = child.kill().await;
                let _ = child.wait().await;
            }
            await_output_tasks(stdout_task.take(), stderr_task.take()).await;
            finish_failed(&job, None, "execution timeout elapsed".into()).await;
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

fn prepend_workspace_gobin_to_path(environment: &mut BTreeMap<String, String>) {
    let Some(gobin) = environment.get("GOBIN").cloned() else {
        return;
    };
    let inherited = environment
        .get("PATH")
        .map(std::ffi::OsString::from)
        .unwrap_or_else(|| std::env::var_os("PATH").unwrap_or_default());
    if let Ok(path) = std::env::join_paths(
        std::iter::once(PathBuf::from(gobin)).chain(std::env::split_paths(&inherited)),
    ) {
        environment.insert("PATH".into(), path.to_string_lossy().into_owned());
    }
}

/// Repository directories inside a workspace.
///
/// Returns `Err` when the workspace cannot be enumerated, rather than an empty
/// list. The difference matters: an empty list means "this workspace has no
/// repositories", a failed read means "this node could not tell" — and on a
/// network mount the second is routine. Collapsing them would let a preflight
/// that walks this list conclude there is nothing to check and admit work into a
/// workspace it never inspected.
async fn discover_repo_names(workspace: &Path) -> std::io::Result<Vec<String>> {
    let mut names = Vec::new();
    let mut entries = tokio::fs::read_dir(workspace).await?;
    while let Some(entry) = entries.next_entry().await? {
        // Only directories can be repositories. Test this *before* joining
        // `.git`: on a regular file — and a workspace root routinely holds
        // `CLAUDE.md`, `AGENTS.md` and copied attachments — `<file>/.git`
        // stats as `NotADirectory`, an error rather than "absent", which would
        // otherwise fail enumeration and take every dispatch down with it.
        if !entry.file_type().await?.is_dir() {
            continue;
        }
        let path = entry.path();
        if std::fs::exists(path.join(".git"))?
            && let Some(name) = entry.file_name().to_str()
        {
            names.push(name.to_owned());
        }
    }
    names.sort();
    Ok(names)
}

/// Refuse a dispatch whose workspace holds a worktree this node cannot use.
///
/// The worker deliberately cannot fix anything here — worktree administration
/// belongs to the coordinator — but it is the only participant that can tell
/// whether a worktree resolves *on this node*, which is the question that
/// actually matters. Refusing costs a dispatch; accepting costs an agent turn,
/// because the failure surfaces as `fatal: not a git repository` several tool
/// calls in.
async fn verify_worktrees_are_usable(
    workspace_path: &Path,
    shared_root: &Path,
) -> Result<(), ExecutionError> {
    let repo_names = discover_repo_names(workspace_path).await.map_err(|e| {
        ExecutionError::WorktreeUnusable {
            detail: format!(
                "could not enumerate repositories in {}: {e}",
                workspace_path.display()
            ),
        }
    })?;

    let mut unusable = Vec::new();
    for name in repo_names {
        // A `.recovered-<epoch>` sibling is a copy the coordinator moved aside
        // to preserve work; its registration is deliberately gone. It is
        // evidence of a past rescue, not a workspace the agent is meant to use,
        // and refusing dispatch over one would strand the workspace forever.
        if name.contains(".recovered-") {
            continue;
        }
        let repo_path = workspace_path.join(&name);
        match WorktreeLinkage::probe(&repo_path, shared_root) {
            // `OwnRepository` is a repository someone put in the workspace, not
            // a managed worktree; it is not this check's business.
            LinkageStatus::Portable { .. } | LinkageStatus::OwnRepository => {}
            other => unusable.push(format!("{name}: {}", other.describe())),
        }
    }

    if unusable.is_empty() {
        Ok(())
    } else {
        Err(ExecutionError::WorktreeUnusable {
            detail: unusable.join("; "),
        })
    }
}

async fn await_output_tasks(
    stdout: Option<tokio::task::JoinHandle<()>>,
    stderr: Option<tokio::task::JoinHandle<()>>,
) {
    if let Some(stdout) = stdout {
        let _ = stdout.await;
    }
    if let Some(stderr) = stderr {
        let _ = stderr.await;
    }
}

async fn stream_output(job: Arc<WorkerJob>, mut reader: impl AsyncRead + Unpin, stderr: bool) {
    let mut buffer = vec![0_u8; 8 * 1024];
    loop {
        match reader.read(&mut buffer).await {
            Ok(0) => return,
            Ok(size) => {
                let data_base64 = BASE64_STANDARD.encode(&buffer[..size]);
                let payload = if stderr {
                    ExecutionEventPayload::Stderr { data_base64 }
                } else {
                    ExecutionEventPayload::Stdout { data_base64 }
                };
                let _ = job.journal.lock().await.append(SystemTime::now(), payload);
            }
            Err(error) => {
                let payload = ExecutionEventPayload::Structured {
                    json: serde_json::json!({"stream_error": error.to_string()}).to_string(),
                };
                let _ = job.journal.lock().await.append(SystemTime::now(), payload);
                return;
            }
        }
    }
}

async fn set_state(job: &WorkerJob, state: JobState, payload: ExecutionEventPayload) {
    job.transition(state, payload).await;
}

async fn finish_status(job: &WorkerJob, status: ExitStatus) {
    let terminal_state = if job.state().await == JobState::Cancelling {
        TerminalState::Killed
    } else if status.success() {
        TerminalState::Completed
    } else {
        TerminalState::Failed
    };
    let evidence = terminal_evidence(terminal_state.clone(), status.code(), signal(&status));
    let (state, payload) = match terminal_state {
        TerminalState::Completed => (
            JobState::Completed,
            ExecutionEventPayload::Completed(evidence),
        ),
        TerminalState::Killed => (JobState::Killed, ExecutionEventPayload::Killed(evidence)),
        _ => (JobState::Failed, ExecutionEventPayload::Failed(evidence)),
    };
    set_state(job, state, payload).await;
    *job.mcp_refresh.write().await = None;
    *job.mcp_config.lock().await = None;
}

async fn finish_failed(job: &WorkerJob, exit_code: Option<i32>, reason: String) {
    let _ = job.journal.lock().await.append(
        SystemTime::now(),
        ExecutionEventPayload::Structured {
            json: serde_json::json!({"worker_error": reason}).to_string(),
        },
    );
    let evidence = terminal_evidence(TerminalState::Failed, exit_code, None);
    set_state(
        job,
        JobState::Failed,
        ExecutionEventPayload::Failed(evidence),
    )
    .await;
    *job.mcp_refresh.write().await = None;
    *job.mcp_config.lock().await = None;
}

fn terminal_evidence(
    state: TerminalState,
    exit_code: Option<i32>,
    signal: Option<i32>,
) -> TerminalEvidence {
    TerminalEvidence {
        state,
        exit_code,
        signal,
        observed_at: Utc::now(),
    }
}

fn authorize_working_directory(
    workspace_path: &Path,
    requested: &str,
    authority: &PathAuthority,
) -> Result<PathBuf, ExecutionError> {
    let requested = Path::new(requested);
    let candidate = if requested.is_absolute() {
        requested.to_owned()
    } else {
        workspace_path.join(requested)
    };
    let canonical = authority.authorize_workspace_path(candidate)?;
    if !canonical.starts_with(workspace_path) {
        return Err(ExecutionError::WorkingDirectoryOutsideWorkspace);
    }
    Ok(canonical)
}

#[cfg(unix)]
fn signal(status: &ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
fn signal(_status: &ExitStatus) -> Option<i32> {
    None
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        fs,
        sync::atomic::{AtomicUsize, Ordering as AtomicOrdering},
    };

    use async_trait::async_trait;
    use cluster_protocol::{PROTOCOL_VERSION, PersistencePolicy, RequestAuthority};
    use executors::mcp_refresh::{
        McpRefreshControl, McpRefreshErrorCategory, McpServerRefreshSnapshot,
        McpServerRefreshStatus,
    };
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn dispatched_gobin_is_prepended_to_worker_path() {
        let mut environment = BTreeMap::from([
            (
                "GOBIN".into(),
                "/shared/workspace/.vibe-kanban/go/bin".into(),
            ),
            ("PATH".into(), "/worker/nix/bin:/usr/bin".into()),
        ]);

        prepend_workspace_gobin_to_path(&mut environment);

        let paths: Vec<_> =
            std::env::split_paths(std::ffi::OsStr::new(&environment["PATH"])).collect();
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/shared/workspace/.vibe-kanban/go/bin"),
                PathBuf::from("/worker/nix/bin"),
                PathBuf::from("/usr/bin"),
            ]
        );
    }

    fn fixture() -> (TempDir, ExecutionSupervisor, PathBuf) {
        let temp = TempDir::new().unwrap();
        let shared = temp.path().join("shared");
        let workspace = shared.join("workspaces").join(Uuid::new_v4().to_string());
        fs::create_dir_all(&workspace).unwrap();
        let authority = PathAuthority::new(&shared).unwrap();
        let supervisor = ExecutionSupervisor::with_runtime_config(
            authority,
            temp.path().join("worker-state/mcp-config"),
            reqwest::Url::parse("http://coordinator.internal:3334/base").unwrap(),
        );
        (temp, supervisor, workspace)
    }

    fn dispatch(workspace: &Path, digest: &str, script: &str) -> ExecutionDispatch {
        let execution_id = Uuid::new_v4();
        ExecutionDispatch {
            authority: RequestAuthority {
                protocol_version: PROTOCOL_VERSION,
                coordinator_id: Uuid::new_v4(),
                worker_node_id: Uuid::new_v4(),
                correlation_id: Uuid::new_v4(),
                issued_at: Utc::now(),
                nonce: Uuid::new_v4().to_string(),
            },
            execution_id,
            workspace_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            workspace_path: workspace.to_string_lossy().into_owned(),
            working_directory: ".".into(),
            executor_profile: "fixture".into(),
            executor_profile_config: None,
            mcp_config_snapshot: None,
            action: json!({"program": "/bin/sh", "args": ["-c", script]}),
            environment: BTreeMap::new(),
            run_reason: "test".into(),
            timeout_seconds: Some(5),
            persistence: PersistencePolicy::Ordinary,
            request_digest: digest.into(),
        }
    }

    async fn wait_terminal(supervisor: &ExecutionSupervisor, execution_id: Uuid) -> JobSummary {
        for _ in 0..200 {
            let summary = supervisor
                .inventory()
                .await
                .into_iter()
                .find(|summary| summary.execution_id == execution_id)
                .unwrap();
            if summary.state.is_terminal() {
                return summary;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("execution did not finish");
    }

    #[derive(Default)]
    struct RefreshFixture {
        queued: AtomicUsize,
    }

    #[async_trait]
    impl McpRefreshControl for RefreshFixture {
        async fn queue_refresh(&self) -> Result<(), McpRefreshErrorCategory> {
            self.queued.fetch_add(1, AtomicOrdering::SeqCst);
            Ok(())
        }

        async fn list_servers(
            &self,
        ) -> Result<Vec<McpServerRefreshSnapshot>, McpRefreshErrorCategory> {
            Ok(vec![McpServerRefreshSnapshot {
                server_id: "snapshot-b".into(),
                status: McpServerRefreshStatus::Ready,
                tool_count: Some(1),
                resource_count: Some(0),
                prompt_count: Some(0),
                restart_occurred: Some(true),
                error: None,
            }])
        }
    }

    #[test]
    fn runtime_gateway_urls_use_the_worker_coordinator_authority() {
        let coordinator = reqwest::Url::parse("http://coordinator.internal:3334/base/").unwrap();
        let snapshot = McpConfigSnapshot {
            executor: BaseCodingAgent::Codex.to_string(),
            servers: BTreeMap::from([
                (
                    "ipv4".into(),
                    json!({
                        "url": "http://127.0.0.1:3334/mcp-gateway/connection-a",
                        "http_headers": {"Authorization": "Bearer fixture-capability"}
                    }),
                ),
                (
                    "ipv6".into(),
                    json!({"url": "http://[::1]:3334/mcp-gateway/connection-b"}),
                ),
                (
                    "direct".into(),
                    json!({"url": "https://mcp.example.test/mcp"}),
                ),
            ]),
        };

        let servers = runtime_mcp_servers(&snapshot, &coordinator);
        assert_eq!(
            servers["ipv4"]["url"],
            "http://coordinator.internal:3334/base/mcp-gateway/connection-a"
        );
        assert_eq!(
            servers["ipv4"]["http_headers"]["Authorization"],
            "Bearer fixture-capability"
        );
        assert_eq!(
            servers["ipv6"]["url"],
            "http://coordinator.internal:3334/base/mcp-gateway/connection-b"
        );
        assert_eq!(servers["direct"]["url"], "https://mcp.example.test/mcp");
    }

    #[test]
    fn gateway_adapter_preserves_non_loopback_and_unrecognized_urls() {
        let coordinator = reqwest::Url::parse("https://coordinator.example.test").unwrap();
        for configured in [
            "https://mcp.example.test/mcp-gateway/connection",
            "http://127.0.0.1:3334/not-the-gateway",
            "not a URL",
        ] {
            assert_eq!(runtime_gateway_url(configured, &coordinator), None);
        }
    }

    #[test]
    fn execution_scoped_mcp_roots_are_uuid_isolated_under_worker_state() {
        let (temp, supervisor, _workspace) = fixture();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let expected_root = temp.path().join("worker-state/mcp-config");

        assert_eq!(supervisor.mcp_config_root, expected_root);
        assert_ne!(
            supervisor.mcp_config_root.join(first.to_string()),
            supervisor.mcp_config_root.join(second.to_string())
        );
        assert!(
            !supervisor
                .mcp_config_root
                .starts_with(std::env::temp_dir().join("vibe-kanban"))
        );
    }

    #[tokio::test]
    async fn refresh_replaces_only_scoped_mcp_snapshot_and_preserves_live_job() {
        let (_temp, supervisor, _workspace) = fixture();
        let execution_id = Uuid::new_v4();
        let scoped_home = std::env::temp_dir()
            .join("vibe-kanban-worker-refresh-test")
            .join(execution_id.to_string())
            .join("codex");
        fs::create_dir_all(&scoped_home).unwrap();
        fs::write(
            scoped_home.join("config.toml"),
            "model = 'preserved'\n[mcp_servers.snapshot-a]\ncommand = 'old'\n",
        )
        .unwrap();
        fs::write(scoped_home.join("history.jsonl"), "conversation-state").unwrap();
        let control = Arc::new(RefreshFixture::default());
        let agent: CodingAgent = serde_json::from_value(json!({"CODEX": {}})).unwrap();
        let job = Arc::new(WorkerJob {
            execution_id,
            worker_job_id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            request_digest: "refresh-fixture".into(),
            state: RwLock::new(JobState::Running),
            journal: Mutex::new(EventJournal::new(execution_id, 16).unwrap()),
            child: Mutex::new(None),
            cancellation: Mutex::new(()),
            acknowledged_sequence: Mutex::new(0),
            recovery_store: None,
            interactions: Arc::new(InteractionBroker::default()),
            mcp_config: Mutex::new(Some(PreparedMcpConfig {
                execution_root: scoped_home.parent().unwrap().to_path_buf(),
                target_config: scoped_home.join("config.toml"),
                environment: std::collections::BTreeMap::from([(
                    "CODEX_HOME".into(),
                    scoped_home.to_string_lossy().into_owned(),
                )]),
                agent,
            })),
            mcp_refresh: RwLock::new(Some(McpRefreshHandle(control.clone()))),
            mcp_refresh_claim: Mutex::new(()),
            quiesced_by: Mutex::new(None),
        });
        supervisor.jobs.write().await.insert(execution_id, job);

        let result = supervisor
            .refresh_mcp(
                execution_id,
                &McpConfigSnapshot {
                    executor: BaseCodingAgent::Codex.to_string(),
                    servers: BTreeMap::from([(
                        "snapshot-b".into(),
                        json!({
                            "url": "http://127.0.0.1:3334/mcp-gateway/connection-b",
                            "http_headers": {"Authorization": "Bearer fixture-capability"}
                        }),
                    )]),
                },
            )
            .await
            .unwrap();

        assert_eq!(result.status, WorkerMcpRefreshStatus::Queued);
        assert_eq!(control.queued.load(AtomicOrdering::SeqCst), 1);
        let config = fs::read_to_string(scoped_home.join("config.toml")).unwrap();
        assert!(config.contains("model = \"preserved\""));
        assert!(config.contains("snapshot-b"));
        assert!(!config.contains("snapshot-a"));
        assert!(config.contains("http://coordinator.internal:3334/base/mcp-gateway/connection-b"));
        assert_eq!(
            fs::read_to_string(scoped_home.join("history.jsonl")).unwrap(),
            "conversation-state"
        );
        let status = supervisor.mcp_status(execution_id).await.unwrap();
        assert_eq!(status.servers[0]["server_id"], "snapshot-b");
        assert_eq!(
            supervisor.job(execution_id).await.unwrap().state().await,
            JobState::Running
        );
    }

    #[tokio::test]
    async fn duplicate_digest_reuses_job_and_conflict_never_starts() {
        let (_temp, supervisor, workspace) = fixture();
        let first = dispatch(&workspace, "same", "printf once");
        let execution_id = first.execution_id;
        let accepted = supervisor.dispatch(first.clone()).await.unwrap();
        let replay = supervisor.dispatch(first).await.unwrap();
        assert_eq!(replay.worker_job_id, accepted.worker_job_id);

        let mut conflict = dispatch(&workspace, "different", "printf duplicate");
        conflict.execution_id = execution_id;
        assert!(matches!(
            supervisor.dispatch(conflict).await,
            Err(ExecutionError::DigestConflict { .. })
        ));
        assert_eq!(supervisor.inventory().await.len(), 1);
    }

    #[tokio::test]
    async fn streams_stdout_stderr_and_records_terminal_evidence() {
        let (_temp, supervisor, workspace) = fixture();
        let request = dispatch(&workspace, "output", "printf hello; printf error >&2");
        let execution_id = request.execution_id;
        supervisor.dispatch(request).await.unwrap();
        let summary = wait_terminal(&supervisor, execution_id).await;
        assert_eq!(summary.state, JobState::Completed);
        assert_eq!(summary.terminal.unwrap().exit_code, Some(0));
        let batch = supervisor.events(execution_id, 0).await.unwrap();
        assert!(
            batch
                .events
                .iter()
                .any(|event| matches!(event.payload, ExecutionEventPayload::Stdout { .. }))
        );
        assert!(
            batch
                .events
                .iter()
                .any(|event| matches!(event.payload, ExecutionEventPayload::Stderr { .. }))
        );
    }

    #[test]
    fn scoped_homes_share_runtime_assets_but_not_config_files() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let first = temp.path().join("scoped").join("first").join("codex");
        let second = temp.path().join("scoped").join("second").join("codex");
        fs::create_dir_all(source.join("skills")).unwrap();
        fs::write(source.join("auth.json"), "credential").unwrap();
        fs::write(source.join("config.toml"), "global = true").unwrap();
        fs::write(source.join("config.toml.bak"), "global backup").unwrap();

        prepare_scoped_home(&source, &first, Path::new("config.toml")).unwrap();
        prepare_scoped_home(&source, &second, Path::new("config.toml")).unwrap();
        fs::write(first.join("config.toml"), "snapshot = 'one'").unwrap();
        fs::write(second.join("config.toml"), "snapshot = 'two'").unwrap();

        assert_eq!(
            fs::read_to_string(first.join("auth.json")).unwrap(),
            "credential"
        );
        assert!(first.join("skills").is_dir());
        assert_eq!(
            fs::read_to_string(first.join("config.toml")).unwrap(),
            "snapshot = 'one'"
        );
        assert_eq!(
            fs::read_to_string(second.join("config.toml")).unwrap(),
            "snapshot = 'two'"
        );
        assert_eq!(
            fs::read_to_string(source.join("config.toml")).unwrap(),
            "global = true"
        );
        assert!(!first.join("config.toml.bak").exists());
    }

    #[test]
    fn scoped_home_can_start_without_a_global_home() {
        let temp = TempDir::new().unwrap();
        let missing_source = temp.path().join("missing");
        let scoped = temp.path().join("scoped").join("codex");

        prepare_scoped_home(&missing_source, &scoped, Path::new("config.toml")).unwrap();

        assert!(scoped.is_dir());
    }

    #[test]
    fn scoped_home_isolates_nested_config_and_preserves_vendor_auth() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let scoped = temp.path().join("scoped").join("home");
        fs::create_dir_all(source.join(".gemini")).unwrap();
        fs::write(source.join(".gemini/oauth_creds.json"), "credential").unwrap();
        fs::write(source.join(".gemini/settings.json"), "global-settings").unwrap();
        fs::write(source.join(".gitconfig"), "git-settings").unwrap();

        prepare_scoped_home(&source, &scoped, Path::new(".gemini/settings.json")).unwrap();
        fs::write(scoped.join(".gemini/settings.json"), "session-settings").unwrap();

        assert_eq!(
            fs::read_to_string(scoped.join(".gemini/oauth_creds.json")).unwrap(),
            "credential"
        );
        assert_eq!(
            fs::read_to_string(scoped.join(".gitconfig")).unwrap(),
            "git-settings"
        );
        assert_eq!(
            fs::read_to_string(source.join(".gemini/settings.json")).unwrap(),
            "global-settings"
        );
    }

    #[test]
    fn custom_xdg_config_outside_home_gets_its_own_scoped_root() {
        let temp = TempDir::new().unwrap();
        let source_home = temp.path().join("home");
        let source_xdg = temp.path().join("config");
        let source_config = source_xdg.join("opencode/opencode.json");
        let execution_root = temp.path().join("execution");

        let (source_root, scoped_root, relative, environment) = non_codex_scoped_config_layout(
            &source_config,
            source_home,
            Some(source_xdg.clone()),
            &execution_root,
        )
        .unwrap();

        assert_eq!(source_root, source_xdg);
        assert_eq!(scoped_root, execution_root.join("xdg"));
        assert_eq!(relative, Path::new("opencode/opencode.json"));
        assert_eq!(
            environment.get("XDG_CONFIG_HOME"),
            Some(&execution_root.join("xdg").to_string_lossy().into_owned())
        );
        assert!(!environment.contains_key("HOME"));
    }

    #[tokio::test]
    async fn raw_command_preserves_dispatched_path() {
        let (_temp, supervisor, workspace) = fixture();
        let mut request = dispatch(&workspace, "path", "printf %s \"$PATH\"");
        request
            .environment
            .insert("PATH".into(), "/fixture/bin".into());
        let execution_id = request.execution_id;
        supervisor.dispatch(request).await.unwrap();
        let summary = wait_terminal(&supervisor, execution_id).await;
        assert_eq!(summary.state, JobState::Completed);

        let batch = supervisor.events(execution_id, 0).await.unwrap();
        let output = batch
            .events
            .iter()
            .filter_map(|event| match &event.payload {
                ExecutionEventPayload::Stdout { data_base64 } => {
                    BASE64_STANDARD.decode(data_base64).ok()
                }
                _ => None,
            })
            .flatten()
            .collect::<Vec<_>>();
        assert!(String::from_utf8_lossy(&output).starts_with("/fixture/bin"));
    }

    #[tokio::test]
    async fn active_count_is_authoritative_drain_evidence() {
        let (_temp, supervisor, workspace) = fixture();
        assert_eq!(supervisor.active_execution_count().await, 0);

        let request = dispatch(&workspace, "drain", "sleep 0.2");
        supervisor.dispatch(request).await.unwrap();
        assert_eq!(supervisor.active_execution_count().await, 1);

        for _ in 0..100 {
            if supervisor.active_execution_count().await == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(supervisor.active_execution_count().await, 0);
    }

    #[tokio::test]
    async fn quarantined_live_process_remains_active_drain_evidence() {
        let (_temp, supervisor, workspace) = fixture();
        let request = dispatch(&workspace, "quarantined-live", "sleep 0.2");
        let execution_id = request.execution_id;
        supervisor.dispatch(request).await.unwrap();

        for _ in 0..100 {
            if supervisor.job(execution_id).await.unwrap().state().await == JobState::Running {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        supervisor.quarantine(execution_id).await.unwrap();
        assert_eq!(supervisor.active_execution_count().await, 1);

        for _ in 0..100 {
            if supervisor.active_execution_count().await == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(supervisor.active_execution_count().await, 0);
    }

    #[tokio::test]
    async fn drain_refuses_new_dispatch_but_keeps_idempotent_retries() {
        let (temp, _fixture_supervisor, workspace) = fixture();
        let draining = Arc::new(AtomicBool::new(false));
        let supervisor = ExecutionSupervisor::with_recovery_and_drain(
            PathAuthority::new(temp.path().join("shared")).unwrap(),
            RecoveryStore::new(temp.path().join("state")).await.unwrap(),
            draining.clone(),
            temp.path().join("state/mcp-config"),
            reqwest::Url::parse("http://coordinator:3334").unwrap(),
        )
        .await
        .unwrap();
        let accepted = dispatch(&workspace, "accepted-before-drain", "sleep 0.2");
        supervisor.dispatch(accepted.clone()).await.unwrap();

        draining.store(true, Ordering::Release);
        assert!(supervisor.dispatch(accepted).await.is_ok());
        assert!(matches!(
            supervisor
                .dispatch(dispatch(&workspace, "new-during-drain", "true"))
                .await,
            Err(ExecutionError::Draining)
        ));
    }

    #[tokio::test]
    async fn coordinator_polling_gap_does_not_interrupt_owned_execution() {
        let (_temp, supervisor, workspace) = fixture();
        let request = dispatch(
            &workspace,
            "coordinator-restart",
            "printf before; sleep 0.1; printf after",
        );
        let execution_id = request.execution_id;
        supervisor.dispatch(request).await.unwrap();

        // Simulate the coordinator being absent: no events or acknowledgements
        // are requested while the worker-owned process continues to run.
        tokio::time::sleep(Duration::from_millis(200)).await;

        let summary = wait_terminal(&supervisor, execution_id).await;
        assert_eq!(summary.state, JobState::Completed);
        let output = supervisor
            .events(execution_id, 0)
            .await
            .unwrap()
            .events
            .into_iter()
            .filter_map(|event| match event.payload {
                ExecutionEventPayload::Stdout { data_base64 } => {
                    BASE64_STANDARD.decode(data_base64).ok()
                }
                _ => None,
            })
            .flatten()
            .collect::<Vec<_>>();
        assert_eq!(String::from_utf8(output).unwrap(), "beforeafter");
    }

    #[tokio::test]
    async fn rejects_working_directory_outside_workspace() {
        let (temp, supervisor, workspace) = fixture();
        let outside = temp.path().join("shared").join("other");
        fs::create_dir(&outside).unwrap();
        let mut request = dispatch(&workspace, "escape", "true");
        request.working_directory = outside.to_string_lossy().into_owned();
        assert!(matches!(
            supervisor.dispatch(request).await,
            Err(ExecutionError::WorkingDirectoryOutsideWorkspace)
        ));
    }

    #[tokio::test]
    async fn restart_retains_terminal_jobs_and_interrupts_unverified_active_jobs() {
        let temp = TempDir::new().unwrap();
        let shared = temp.path().join("shared");
        fs::create_dir_all(&shared).unwrap();
        let store = RecoveryStore::new(temp.path().join("state")).await.unwrap();
        let active = JobSummary {
            execution_id: Uuid::new_v4(),
            worker_job_id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            request_digest: "active".into(),
            state: JobState::Running,
            last_sequence: 4,
            terminal: None,
        };
        let terminal_evidence = terminal_evidence(TerminalState::Completed, Some(0), None);
        let completed = JobSummary {
            execution_id: Uuid::new_v4(),
            worker_job_id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            request_digest: "completed".into(),
            state: JobState::Completed,
            last_sequence: 7,
            terminal: Some(terminal_evidence.clone()),
        };
        store.save(&active).await.unwrap();
        store.save(&completed).await.unwrap();

        let supervisor = ExecutionSupervisor::with_recovery(
            PathAuthority::new(&shared).unwrap(),
            store,
            temp.path().join("state/mcp-config"),
            reqwest::Url::parse("http://coordinator:3334").unwrap(),
        )
        .await
        .unwrap();
        let inventory = supervisor.inventory().await;
        let recovered_active = inventory
            .iter()
            .find(|job| job.execution_id == active.execution_id)
            .unwrap();
        assert_eq!(recovered_active.state, JobState::Interrupted);
        assert_eq!(
            recovered_active.terminal.as_ref().unwrap().state,
            TerminalState::Interrupted
        );
        let recovered_completed = inventory
            .iter()
            .find(|job| job.execution_id == completed.execution_id)
            .unwrap();
        assert_eq!(recovered_completed.state, JobState::Completed);
        assert_eq!(recovered_completed.terminal, Some(terminal_evidence));
    }
}

#[cfg(test)]
mod worktree_preflight_tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    /// A workspace root routinely holds plain files — `CLAUDE.md` and
    /// `AGENTS.md` are written there by workspace provisioning, and attachments
    /// are copied there. Enumeration must step over them.
    ///
    /// It is worth a test of its own because getting it wrong is not a partial
    /// failure: `<file>/.git` stats as `NotADirectory` rather than "absent", so
    /// a single stray file at the workspace root would fail enumeration and
    /// make the preflight refuse *every* dispatch to that workspace.
    #[tokio::test]
    async fn files_in_the_workspace_root_do_not_break_enumeration() {
        let fixture = TempDir::new().unwrap();
        let workspace = fixture.path();
        fs::write(workspace.join("CLAUDE.md"), "# guidance\n").unwrap();
        fs::write(workspace.join("attachment.png"), b"not a repo").unwrap();
        fs::create_dir_all(workspace.join("plain-dir")).unwrap();
        fs::create_dir_all(workspace.join("repo")).unwrap();
        fs::write(workspace.join("repo").join(".git"), "gitdir: /elsewhere\n").unwrap();

        let names = discover_repo_names(workspace).await.unwrap();

        assert_eq!(names, vec!["repo".to_string()]);
    }

    #[tokio::test]
    async fn a_workspace_that_cannot_be_enumerated_is_refused_not_assumed_empty() {
        let fixture = TempDir::new().unwrap();
        let missing = fixture.path().join("gone");

        let error = verify_worktrees_are_usable(&missing, fixture.path())
            .await
            .expect_err("an unreadable workspace must not be treated as having no repositories");

        assert!(matches!(error, ExecutionError::WorktreeUnusable { .. }));
    }

    /// The production defect, seen from the worker: a worktree pointing at
    /// storage only the coordinator can reach.
    #[tokio::test]
    async fn refuses_a_workspace_whose_worktree_points_outside_the_shared_root() {
        let fixture = TempDir::new().unwrap();
        let shared_root = fixture.path().join("shared");
        let workspace = shared_root.join("workspaces").join("ws-1");
        fs::create_dir_all(workspace.join("repo")).unwrap();
        fs::write(
            workspace.join("repo").join(".git"),
            "gitdir: /srv/src/repo/.git/worktrees/repo\n",
        )
        .unwrap();

        let error = verify_worktrees_are_usable(&workspace, &shared_root)
            .await
            .expect_err("a dangling worktree must be refused before any work starts");

        match error {
            ExecutionError::WorktreeUnusable { detail } => {
                assert!(detail.contains("repo"), "{detail}")
            }
            other => panic!("expected WorktreeUnusable, got {other:?}"),
        }
    }

    /// A `.recovered-<epoch>` sibling is preserved evidence of a past rescue,
    /// not a worktree the agent uses. Refusing over one would strand the
    /// workspace permanently, since the worker cannot repair anything.
    #[tokio::test]
    async fn ignores_preserved_recovery_directories() {
        let fixture = TempDir::new().unwrap();
        let shared_root = fixture.path().join("shared");
        let workspace = shared_root.join("workspaces").join("ws-1");
        fs::create_dir_all(workspace.join("repo.recovered-1712000000")).unwrap();
        fs::write(
            workspace.join("repo.recovered-1712000000").join(".git"),
            "gitdir: /srv/src/repo/.git/worktrees/repo\n",
        )
        .unwrap();
        fs::create_dir_all(&shared_root).unwrap();

        verify_worktrees_are_usable(&workspace, &shared_root)
            .await
            .expect("a preserved recovery directory must not block dispatch");
    }
}
