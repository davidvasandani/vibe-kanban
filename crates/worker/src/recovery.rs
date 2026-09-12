use std::path::{Path, PathBuf};

use cluster_protocol::JobSummary;
use thiserror::Error;
use tokio::fs;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum RecoveryError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct RecoveryStore {
    root: PathBuf,
}

impl RecoveryStore {
    pub async fn new(root: impl AsRef<Path>) -> Result<Self, RecoveryError> {
        let root = root.as_ref().to_owned();
        fs::create_dir_all(&root).await?;
        Ok(Self { root })
    }

    pub async fn save(&self, summary: &JobSummary) -> Result<(), RecoveryError> {
        let target = self.path(summary.execution_id);
        let temporary = target.with_extension(format!("json.tmp-{}", Uuid::new_v4()));
        fs::write(&temporary, serde_json::to_vec(summary)?).await?;
        fs::rename(temporary, target).await?;
        Ok(())
    }

    pub async fn load(&self) -> Result<Vec<JobSummary>, RecoveryError> {
        let mut summaries: Vec<JobSummary> = Vec::new();
        let mut entries = fs::read_dir(&self.root).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            summaries.push(serde_json::from_slice(&fs::read(entry.path()).await?)?);
        }
        summaries.sort_by_key(|summary| summary.execution_id);
        Ok(summaries)
    }

    fn path(&self, execution_id: Uuid) -> PathBuf {
        self.root.join(format!("{execution_id}.json"))
    }
}

#[cfg(test)]
mod tests {
    use cluster_protocol::{JobState, TerminalEvidence, TerminalState};
    use tempfile::TempDir;

    use super::*;

    #[tokio::test]
    async fn atomically_round_trips_retained_inventory() {
        let temp = TempDir::new().unwrap();
        let store = RecoveryStore::new(temp.path()).await.unwrap();
        let summary = JobSummary {
            execution_id: Uuid::new_v4(),
            worker_job_id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            request_digest: "digest".into(),
            state: JobState::Completed,
            last_sequence: 9,
            terminal: Some(TerminalEvidence {
                state: TerminalState::Completed,
                exit_code: Some(0),
                signal: None,
                observed_at: chrono::Utc::now(),
            }),
        };
        store.save(&summary).await.unwrap();
        assert_eq!(store.load().await.unwrap(), vec![summary]);
    }
}
