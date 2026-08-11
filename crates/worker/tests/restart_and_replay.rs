use std::time::SystemTime;

use cluster_protocol::{
    ExecutionEventPayload, JobState, JobSummary, TerminalEvidence, TerminalState,
};
use tempfile::TempDir;
use uuid::Uuid;
use worker::{
    execution::ExecutionSupervisor, journal::EventJournal, path_authority::PathAuthority,
    recovery::RecoveryStore,
};

#[tokio::test]
async fn restart_reports_active_job_interrupted_and_keeps_terminal_job() {
    let temp = TempDir::new().unwrap();
    let shared = temp.path().join("shared");
    tokio::fs::create_dir_all(&shared).await.unwrap();
    let store = RecoveryStore::new(temp.path().join("state")).await.unwrap();
    let active = summary(JobState::Running, None, 3);
    let completed_evidence = evidence(TerminalState::Completed);
    let completed = summary(JobState::Completed, Some(completed_evidence.clone()), 8);
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

    let active_after_restart = inventory
        .iter()
        .find(|job| job.execution_id == active.execution_id)
        .unwrap();
    assert_eq!(active_after_restart.state, JobState::Interrupted);
    assert_eq!(
        active_after_restart.terminal.as_ref().unwrap().state,
        TerminalState::Interrupted
    );
    let completed_after_restart = inventory
        .iter()
        .find(|job| job.execution_id == completed.execution_id)
        .unwrap();
    assert_eq!(completed_after_restart.state, JobState::Completed);
    assert_eq!(completed_after_restart.terminal, Some(completed_evidence));
}

#[test]
fn stale_cursor_is_reported_instead_of_silently_skipping_output() {
    let mut journal = EventJournal::new(Uuid::new_v4(), 2).unwrap();
    for value in ["one", "two", "three"] {
        journal
            .append(
                SystemTime::now(),
                ExecutionEventPayload::Stdout {
                    data_base64: value.into(),
                },
            )
            .unwrap();
    }
    let replay = journal.replay_after(0);
    assert!(replay.replay_gap);
    assert_eq!(replay.earliest_available, 2);
}

fn summary(state: JobState, terminal: Option<TerminalEvidence>, last_sequence: u64) -> JobSummary {
    JobSummary {
        execution_id: Uuid::new_v4(),
        worker_job_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        request_digest: Uuid::new_v4().to_string(),
        state,
        last_sequence,
        terminal,
    }
}

fn evidence(state: TerminalState) -> TerminalEvidence {
    TerminalEvidence {
        state,
        exit_code: Some(0),
        signal: None,
        observed_at: chrono::Utc::now(),
    }
}
