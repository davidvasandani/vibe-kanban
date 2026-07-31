use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    sync::Arc,
    time::{Duration, SystemTime},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use chrono::Utc;
use cluster_protocol::{
    DispatchAccepted, EventBatch, ExecutionDispatch, ExecutionEventPayload, JobState, JobSummary,
    TerminalEvidence, TerminalState,
};
use command_group::{AsyncCommandGroup, AsyncGroupChild};
use serde::Deserialize;
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    sync::{Mutex, RwLock},
};
use uuid::Uuid;

use crate::{
    journal::{EventJournal, JournalError},
    path_authority::{PathAuthority, PathAuthorityError},
};

const DEFAULT_JOURNAL_CAPACITY: usize = 4_096;

#[derive(Debug, Deserialize)]
struct WorkerCommandAction {
    program: String,
    #[serde(default)]
    args: Vec<String>,
}

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("execution {execution_id} was already dispatched with a different request digest")]
    DigestConflict { execution_id: Uuid },
    #[error("execution action is invalid: {0}")]
    InvalidAction(#[from] serde_json::Error),
    #[error("execution action program must not be empty")]
    EmptyProgram,
    #[error("working directory resolves outside its authorized workspace")]
    WorkingDirectoryOutsideWorkspace,
    #[error(transparent)]
    PathAuthority(#[from] PathAuthorityError),
    #[error(transparent)]
    Journal(#[from] JournalError),
    #[error("execution {0} was not found")]
    NotFound(Uuid),
}

#[derive(Clone)]
pub struct ExecutionSupervisor {
    path_authority: PathAuthority,
    jobs: Arc<RwLock<HashMap<Uuid, Arc<WorkerJob>>>>,
    journal_capacity: usize,
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
}

impl ExecutionSupervisor {
    pub fn new(path_authority: PathAuthority) -> Self {
        Self::with_journal_capacity(path_authority, DEFAULT_JOURNAL_CAPACITY)
    }

    pub fn with_journal_capacity(path_authority: PathAuthority, journal_capacity: usize) -> Self {
        Self {
            path_authority,
            jobs: Arc::new(RwLock::new(HashMap::new())),
            journal_capacity,
        }
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

        let workspace_path = self
            .path_authority
            .authorize_workspace_path(&dispatch.workspace_path)?;
        let working_directory = authorize_working_directory(
            &workspace_path,
            &dispatch.working_directory,
            &self.path_authority,
        )?;
        let action: WorkerCommandAction = serde_json::from_value(dispatch.action.clone())?;
        if action.program.trim().is_empty() {
            return Err(ExecutionError::EmptyProgram);
        }

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
        });
        jobs.insert(dispatch.execution_id, job.clone());
        drop(jobs);

        tokio::spawn(run_job(
            job.clone(),
            action,
            working_directory,
            dispatch.environment,
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
}

impl WorkerJob {
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
        }
    }
}

async fn run_job(
    job: Arc<WorkerJob>,
    action: WorkerCommandAction,
    working_directory: PathBuf,
    environment: std::collections::BTreeMap<String, String>,
    timeout_seconds: Option<u64>,
) {
    set_state(&job, JobState::Starting, ExecutionEventPayload::Starting).await;
    let mut command = Command::new(action.program);
    command
        .args(action.args)
        .current_dir(working_directory)
        .envs(environment)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = match command.group_spawn() {
        Ok(child) => child,
        Err(error) => {
            finish_failed(&job, None, format!("failed to start process: {error}")).await;
            return;
        }
    };
    let stdout = child.inner().stdout.take();
    let stderr = child.inner().stderr.take();
    *job.child.lock().await = Some(child);
    *job.state.write().await = JobState::Running;

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
    use std::{collections::BTreeMap, fs};

    use cluster_protocol::{PROTOCOL_VERSION, PersistencePolicy, RequestAuthority};
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;

    fn fixture() -> (TempDir, ExecutionSupervisor, PathBuf) {
        let temp = TempDir::new().unwrap();
        let shared = temp.path().join("shared");
        let workspace = shared.join("workspaces").join(Uuid::new_v4().to_string());
        fs::create_dir_all(&workspace).unwrap();
        let authority = PathAuthority::new(&shared).unwrap();
        (temp, ExecutionSupervisor::new(authority), workspace)
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
}
