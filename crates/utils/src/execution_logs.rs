use std::path::{Path, PathBuf};

use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::{assets::asset_dir, log_msg::LogMsg};

pub const EXECUTION_LOGS_DIRNAME: &str = "sessions";

pub fn process_logs_session_dir(session_id: Uuid) -> PathBuf {
    resolve_process_logs_session_dir(&asset_dir(), session_id)
}

pub fn process_log_file_path(session_id: Uuid, process_id: Uuid) -> PathBuf {
    process_log_file_path_in_root(&asset_dir(), session_id, process_id)
}

pub fn process_log_file_path_in_root(root: &Path, session_id: Uuid, process_id: Uuid) -> PathBuf {
    resolve_process_logs_session_dir(root, session_id)
        .join("processes")
        .join(format!("{}.jsonl", process_id))
}

/// Materialized normalized log for a finished process: the settled
/// conversation entries derived from that process's raw log.
///
/// Beside the raw log deliberately — it is a derived view of that exact file,
/// and sharing the session directory means the existing cleanup removes both
/// together rather than leaving a cache describing logs that are gone.
pub fn process_normalized_log_file_path(session_id: Uuid, process_id: Uuid) -> PathBuf {
    process_normalized_log_file_path_in_root(&asset_dir(), session_id, process_id)
}

pub fn process_normalized_log_file_path_in_root(
    root: &Path,
    session_id: Uuid,
    process_id: Uuid,
) -> PathBuf {
    resolve_process_logs_session_dir(root, session_id)
        .join("processes")
        .join(format!("{}.normalized.jsonl", process_id))
}

/// Raw (unstructured) log file that a detached process writes its
/// stdout/stderr to directly, so its output survives a server restart.
/// Used for dev servers, which are left running across restarts.
pub fn process_raw_log_file_path(session_id: Uuid, process_id: Uuid) -> PathBuf {
    process_logs_session_dir(session_id)
        .join("processes")
        .join(format!("{}.raw.log", process_id))
}

pub struct ExecutionLogWriter {
    path: PathBuf,
    file: tokio::fs::File,
}

impl ExecutionLogWriter {
    pub async fn new(path: PathBuf) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        Ok(Self { path, file })
    }

    pub async fn new_for_execution(session_id: Uuid, execution_id: Uuid) -> std::io::Result<Self> {
        Self::new(process_log_file_path(session_id, execution_id)).await
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub async fn append_jsonl_line(&mut self, jsonl_line: &str) -> std::io::Result<()> {
        self.file.write_all(jsonl_line.as_bytes()).await
    }
}

pub async fn read_execution_log_file(path: &Path) -> std::io::Result<String> {
    tokio::fs::read_to_string(path).await
}

pub fn parse_log_jsonl_lossy(execution_id: Uuid, jsonl: &str) -> Vec<LogMsg> {
    let mut messages = Vec::new();
    let mut bad_lines = 0usize;

    for line in jsonl.lines() {
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<LogMsg>(line) {
            Ok(msg) => messages.push(msg),
            Err(e) => {
                bad_lines += 1;
                if bad_lines <= 3 {
                    tracing::warn!(
                        "Skipping unparsable log line for execution {}: {}",
                        execution_id,
                        e
                    );
                }
            }
        }
    }

    if bad_lines > 3 {
        tracing::warn!(
            "Skipped {} unparsable log lines for execution {}",
            bad_lines,
            execution_id
        );
    }

    messages
}

/// Upper bound on how many messages a *historical* (already-finished)
/// execution contributes to normalization.
///
/// Normalizing a finished run holds the parsed messages and every patch
/// derived from them at once, and patch size grows with the length of the
/// conversation so far — so cost climbs faster than linearly with message
/// count. `MsgStore` caps its own history by bytes, but the broadcast ring
/// alongside it is capped by message *count*, so a long run is retained in
/// full regardless of how large those messages are. One 16.5k-message
/// execution reached ~57 GB resident and OOM-killed the server on every
/// reconnect (2026-07-31). Live streaming is unaffected: it never takes this
/// path.
pub const MAX_HISTORICAL_NORMALIZATION_MSGS: usize = 2000;

/// Keep only the newest `max` normalizable messages from a historical run.
///
/// Returns the retained messages (oldest-first, as normalization expects) and
/// how many were elided. Non-normalizable variants are filtered out first, so
/// `max` bounds what actually reaches the store. The tail is kept rather than
/// the head: a log viewer opens on the most recent output.
pub fn cap_normalizable_history(messages: Vec<LogMsg>, max: usize) -> (Vec<LogMsg>, usize) {
    let mut normalizable: Vec<LogMsg> = messages
        .into_iter()
        .filter(|msg| {
            matches!(
                msg,
                LogMsg::Stdout(_) | LogMsg::Stderr(_) | LogMsg::JsonPatch(_)
            )
        })
        .collect();

    let dropped = normalizable.len().saturating_sub(max);
    if dropped > 0 {
        normalizable.drain(..dropped);
    }

    (normalizable, dropped)
}

fn uuid_prefix2(id: Uuid) -> String {
    let s = id.to_string();
    s.chars().take(2).collect()
}

fn resolve_process_logs_session_dir(root: &Path, session_id: Uuid) -> PathBuf {
    root.join(EXECUTION_LOGS_DIRNAME)
        .join(uuid_prefix2(session_id))
        .join(session_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stdout(n: usize) -> LogMsg {
        LogMsg::Stdout(format!("line {n}"))
    }

    fn text(msg: &LogMsg) -> String {
        match msg {
            LogMsg::Stdout(s) | LogMsg::Stderr(s) => s.clone(),
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[test]
    fn keeps_everything_under_the_cap_and_drops_unnormalizable() {
        let messages = vec![stdout(0), LogMsg::Ready, stdout(1), LogMsg::Finished];

        let (kept, dropped) = cap_normalizable_history(messages, 10);

        assert_eq!(dropped, 0);
        assert_eq!(kept.len(), 2, "Ready/Finished are not normalizable");
        assert_eq!(text(&kept[0]), "line 0");
        assert_eq!(text(&kept[1]), "line 1");
    }

    #[test]
    fn keeps_the_newest_messages_when_over_the_cap() {
        let messages: Vec<LogMsg> = (0..100).map(stdout).collect();

        let (kept, dropped) = cap_normalizable_history(messages, 10);

        assert_eq!(dropped, 90);
        assert_eq!(kept.len(), 10);
        // The tail is what a log viewer wants: newest retained, oldest elided.
        assert_eq!(text(&kept[0]), "line 90");
        assert_eq!(text(&kept[9]), "line 99");
    }

    #[test]
    fn a_zero_cap_drops_everything() {
        let (kept, dropped) = cap_normalizable_history(vec![stdout(0), stdout(1)], 0);

        assert!(kept.is_empty());
        assert_eq!(dropped, 2);
    }
}
