use std::{process::ExitStatus, time::Duration};

use cluster_protocol::{
    CancellationPhase, CancellationRequest, CancellationStatus, ExecutionEventPayload, JobState,
    TerminalEvidence, TerminalState,
};
use command_group::AsyncGroupChild;
use thiserror::Error;
use tokio::time::Instant;
use uuid::Uuid;

use crate::execution::{ExecutionSupervisor, WorkerJob};

#[derive(Debug, Error)]
pub enum CancellationError {
    #[error("execution {0} was not found")]
    NotFound(Uuid),
    #[error("failed to signal execution process group: {0}")]
    Signal(#[from] std::io::Error),
}

/// Cancels a job with executor-friendly interrupt, TERM, then KILL. Repeated
/// requests serialize on the job and return its already-confirmed terminal
/// evidence rather than signalling an unrelated later process.
pub async fn cancel(
    supervisor: &ExecutionSupervisor,
    request: &CancellationRequest,
) -> Result<CancellationStatus, CancellationError> {
    let job = supervisor
        .job(request.execution_id)
        .await
        .ok_or(CancellationError::NotFound(request.execution_id))?;
    let _cancellation = job.cancellation_guard().await;

    if let Some(terminal) = job.terminal_evidence().await {
        return Ok(status(
            request.execution_id,
            CancellationPhase::AlreadyTerminal,
            Some(terminal),
        ));
    }

    job.transition(
        JobState::Cancelling,
        structured_phase(CancellationPhase::Requested),
    )
    .await;

    signal_graceful(&job).await?;
    if let Some(exit) = wait_for_exit(
        &job,
        Duration::from_secs(u64::from(request.graceful_timeout_seconds)),
    )
    .await?
    {
        return confirmed(&job, request.execution_id, exit).await;
    }

    job.transition(
        JobState::Cancelling,
        structured_phase(CancellationPhase::TerminatingProcessGroup),
    )
    .await;
    signal_terminate(&job).await?;
    if let Some(exit) = wait_for_exit(
        &job,
        Duration::from_secs(u64::from(request.terminate_timeout_seconds)),
    )
    .await?
    {
        return confirmed(&job, request.execution_id, exit).await;
    }

    job.transition(
        JobState::Cancelling,
        structured_phase(CancellationPhase::KillingProcessGroup),
    )
    .await;
    signal_kill(&job).await?;
    if let Some(exit) = wait_for_exit(&job, Duration::from_secs(2)).await? {
        return confirmed(&job, request.execution_id, exit).await;
    }

    job.transition(
        JobState::Indeterminate,
        ExecutionEventPayload::Indeterminate {
            reason: "process-group kill was not confirmed".into(),
        },
    )
    .await;
    Ok(status(
        request.execution_id,
        CancellationPhase::Indeterminate,
        None,
    ))
}

async fn confirmed(
    job: &WorkerJob,
    execution_id: Uuid,
    exit: ExitStatus,
) -> Result<CancellationStatus, CancellationError> {
    let evidence = TerminalEvidence {
        state: TerminalState::Killed,
        exit_code: exit.code(),
        signal: exit_signal(&exit),
        observed_at: chrono::Utc::now(),
    };
    job.transition(
        JobState::Killed,
        ExecutionEventPayload::Killed(evidence.clone()),
    )
    .await;
    let retained = job.terminal_evidence().await.unwrap_or(evidence);
    Ok(status(
        execution_id,
        CancellationPhase::Confirmed,
        Some(retained),
    ))
}

async fn wait_for_exit(
    job: &WorkerJob,
    duration: Duration,
) -> Result<Option<ExitStatus>, CancellationError> {
    let deadline = Instant::now() + duration;
    loop {
        let observed = {
            let mut child = job.child().await;
            match child.as_mut() {
                Some(child) => child.try_wait()?,
                None => return Ok(None),
            }
        };
        if observed.is_some() {
            return Ok(observed);
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn signal_graceful(job: &WorkerJob) -> Result<(), CancellationError> {
    signal(job, SignalKind::Interrupt).await
}

async fn signal_terminate(job: &WorkerJob) -> Result<(), CancellationError> {
    signal(job, SignalKind::Terminate).await
}

async fn signal_kill(job: &WorkerJob) -> Result<(), CancellationError> {
    signal(job, SignalKind::Kill).await
}

enum SignalKind {
    Interrupt,
    Terminate,
    Kill,
}

async fn signal(job: &WorkerJob, signal: SignalKind) -> Result<(), CancellationError> {
    let mut child = job.child().await;
    let Some(child) = child.as_mut() else {
        return Ok(());
    };
    signal_child(child, signal).await?;
    Ok(())
}

#[cfg(unix)]
async fn signal_child(
    child: &mut AsyncGroupChild,
    signal: SignalKind,
) -> Result<(), std::io::Error> {
    use command_group::{Signal, UnixChildExt};
    let signal = match signal {
        SignalKind::Interrupt => Signal::SIGINT,
        SignalKind::Terminate => Signal::SIGTERM,
        SignalKind::Kill => Signal::SIGKILL,
    };
    child.signal(signal)
}

#[cfg(not(unix))]
async fn signal_child(
    child: &mut AsyncGroupChild,
    _signal: SignalKind,
) -> Result<(), std::io::Error> {
    child.kill().await
}

fn structured_phase(phase: CancellationPhase) -> ExecutionEventPayload {
    ExecutionEventPayload::Structured {
        json: serde_json::json!({"cancellation_phase": phase}).to_string(),
    }
}

fn status(
    execution_id: Uuid,
    phase: CancellationPhase,
    terminal: Option<TerminalEvidence>,
) -> CancellationStatus {
    CancellationStatus {
        execution_id,
        phase,
        terminal,
    }
}

#[cfg(unix)]
fn exit_signal(status: &ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
fn exit_signal(_status: &ExitStatus) -> Option<i32> {
    None
}

#[cfg(all(test, unix))]
mod tests {
    use std::{collections::BTreeMap, fs, path::PathBuf};

    use cluster_protocol::{
        ExecutionDispatch, PROTOCOL_VERSION, PersistencePolicy, RequestAuthority,
    };
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;
    use crate::path_authority::PathAuthority;

    fn fixture() -> (TempDir, ExecutionSupervisor, PathBuf) {
        let temp = TempDir::new().unwrap();
        let shared = temp.path().join("shared");
        let workspace = shared.join("workspaces").join(Uuid::new_v4().to_string());
        fs::create_dir_all(&workspace).unwrap();
        let supervisor = ExecutionSupervisor::new(PathAuthority::new(&shared).unwrap());
        (temp, supervisor, workspace)
    }

    fn dispatch(workspace: &std::path::Path, pid_file: &std::path::Path) -> ExecutionDispatch {
        ExecutionDispatch {
            authority: RequestAuthority {
                protocol_version: PROTOCOL_VERSION,
                coordinator_id: Uuid::new_v4(),
                worker_node_id: Uuid::new_v4(),
                correlation_id: Uuid::new_v4(),
                issued_at: chrono::Utc::now(),
                nonce: Uuid::new_v4().to_string(),
            },
            execution_id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            workspace_path: workspace.to_string_lossy().into_owned(),
            working_directory: ".".into(),
            executor_profile: "fixture".into(),
            executor_profile_config: None,
            action: json!({
                "program": "/bin/sh",
                "args": ["-c", format!("sleep 60 & echo $! > '{}'; wait", pid_file.display())]
            }),
            environment: BTreeMap::new(),
            run_reason: "test".into(),
            timeout_seconds: None,
            persistence: PersistencePolicy::Ordinary,
            request_digest: Uuid::new_v4().to_string(),
        }
    }

    #[tokio::test]
    async fn cancellation_kills_child_and_grandchild_and_is_idempotent() {
        let (temp, supervisor, workspace) = fixture();
        let pid_file = temp.path().join("grandchild.pid");
        let dispatch = dispatch(&workspace, &pid_file);
        let execution_id = dispatch.execution_id;
        supervisor.dispatch(dispatch).await.unwrap();
        for _ in 0..100 {
            if pid_file.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let grandchild: i32 = fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let request = CancellationRequest {
            authority: RequestAuthority {
                protocol_version: PROTOCOL_VERSION,
                coordinator_id: Uuid::new_v4(),
                worker_node_id: Uuid::new_v4(),
                correlation_id: Uuid::new_v4(),
                issued_at: chrono::Utc::now(),
                nonce: Uuid::new_v4().to_string(),
            },
            execution_id,
            graceful_timeout_seconds: 0,
            terminate_timeout_seconds: 0,
        };
        let first = cancel(&supervisor, &request).await.unwrap();
        assert_eq!(first.phase, CancellationPhase::Confirmed);
        let repeated = cancel(&supervisor, &request).await.unwrap();
        assert_eq!(repeated.phase, CancellationPhase::AlreadyTerminal);
        assert_eq!(
            supervisor.quarantine(execution_id).await.unwrap().state,
            JobState::Quarantined
        );

        let alive = std::process::Command::new("/bin/sh")
            .args(["-c", &format!("kill -0 {grandchild} 2>/dev/null")])
            .status()
            .unwrap()
            .success();
        assert!(
            !alive,
            "grandchild process must not survive group cancellation"
        );
    }
}
