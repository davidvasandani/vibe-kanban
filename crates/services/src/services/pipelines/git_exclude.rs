//! Per-repository "keep pipeline artifacts out of git" support.
//!
//! Pipeline stages write design records under `specs/vk/<task-id>/`. A repo
//! with `local_pipeline_artifacts` set gets a Vibe Kanban-managed block in its
//! `info/exclude` ignoring that directory, so the records stay in the worktree
//! but never show up as untracked changes or get committed. Clearing the
//! setting removes the block. Excludes do not untrack files a repo already
//! committed.

use std::{
    io::{self, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

const BLOCK_START: &str =
    "# >>> Vibe Kanban: pipeline artifacts kept out of git (repo setting, managed)";
const BLOCK_END: &str = "# <<< Vibe Kanban: pipeline artifacts";
const ARTIFACT_PATTERN: &str = "/specs/vk/";

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Add (`enabled`) or remove the managed pipeline-artifact block in the
/// `info/exclude` of the repository containing `worktree`. Writes only when
/// the content changes. In a linked worktree the exclude file lives in the
/// shared common dir, so the setting applies to every worktree of the repo.
pub fn sync_artifact_exclude(worktree: &Path, enabled: bool) -> io::Result<()> {
    let out = git::GitCli::new()
        .git(worktree, ["rev-parse", "--git-path", "info/exclude"])
        .map_err(io::Error::other)?;
    let rel = out.trim();
    if rel.is_empty() {
        return Err(io::Error::other("git did not report an info/exclude path"));
    }
    // `--git-path` is relative to `worktree` in a plain checkout and absolute
    // in a linked worktree.
    let exclude = worktree.join(rel);
    let existing = match std::fs::read_to_string(&exclude) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            if !enabled {
                return Ok(());
            }
            String::new()
        }
        Err(e) => return Err(e),
    };
    let updated = with_managed_block(&existing, enabled);
    if updated == existing {
        return Ok(());
    }
    write_atomic(&exclude, &updated)
}

/// `text` with every managed block removed and, when `enabled`, one appended.
fn with_managed_block(text: &str, enabled: bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_block = false;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if !in_block && trimmed == BLOCK_START {
            in_block = true;
            continue;
        }
        if in_block {
            if trimmed == BLOCK_END {
                in_block = false;
            }
            continue;
        }
        out.push_str(line);
    }
    if enabled {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(BLOCK_START);
        out.push('\n');
        out.push_str(ARTIFACT_PATTERN);
        out.push('\n');
        out.push_str(BLOCK_END);
        out.push('\n');
    }
    out
}

/// Concurrent workers may sync the same shared exclude file, so never leave a
/// partially written one behind.
fn write_atomic(path: &Path, content: &str) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::other("info/exclude has no parent directory"))?;
    std::fs::create_dir_all(dir)?;
    let nonce = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = dir.join(format!(
        ".exclude.vk-pipeline-{}-{nonce}",
        std::process::id()
    ));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
        String::from_utf8(out.stdout).unwrap()
    }

    fn init_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q"]);
        dir
    }

    fn untracked(dir: &Path) -> String {
        git(dir, &["status", "--porcelain", "--untracked-files=all"])
    }

    #[test]
    fn block_is_added_once_and_removed_without_touching_other_lines() {
        let text = "# user\n*.log";
        let enabled = with_managed_block(text, true);
        assert_eq!(
            enabled,
            format!("# user\n*.log\n{BLOCK_START}\n{ARTIFACT_PATTERN}\n{BLOCK_END}\n")
        );
        assert_eq!(with_managed_block(&enabled, true), enabled);
        assert_eq!(with_managed_block(&enabled, false), "# user\n*.log\n");
        assert_eq!(with_managed_block(text, false), text);
    }

    #[test]
    fn enabled_setting_ignores_artifacts_and_disabling_restores_them() {
        let repo = init_repo();
        let artifact = repo.path().join("specs/vk/task-a/technical-spec.md");
        std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        std::fs::write(&artifact, "spec").unwrap();
        std::fs::write(repo.path().join("specs/keep.md"), "kept").unwrap();
        assert!(untracked(repo.path()).contains("technical-spec.md"));

        sync_artifact_exclude(repo.path(), true).unwrap();
        let status = untracked(repo.path());
        assert!(!status.contains("technical-spec.md"), "{status}");
        assert!(status.contains("specs/keep.md"), "{status}");

        sync_artifact_exclude(repo.path(), false).unwrap();
        assert!(untracked(repo.path()).contains("technical-spec.md"));
    }

    #[test]
    fn linked_worktree_writes_the_shared_exclude() {
        let repo = init_repo();
        git(
            repo.path(),
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "init",
            ],
        );
        let worktree = repo.path().join("wt");
        git(
            repo.path(),
            &["worktree", "add", "-q", worktree.to_str().unwrap()],
        );

        sync_artifact_exclude(&worktree, true).unwrap();

        let shared = std::fs::read_to_string(repo.path().join(".git/info/exclude")).unwrap();
        assert!(shared.contains(ARTIFACT_PATTERN));
        assert_eq!(shared.matches(BLOCK_START).count(), 1);
    }

    #[test]
    fn disabling_without_an_exclude_file_creates_nothing() {
        let repo = init_repo();
        let exclude = repo.path().join(".git/info/exclude");
        let _ = std::fs::remove_file(&exclude);

        sync_artifact_exclude(repo.path(), false).unwrap();

        assert!(!exclude.exists());
    }
}
