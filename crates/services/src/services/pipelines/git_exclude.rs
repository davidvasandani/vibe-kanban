//! Per-repository "keep pipeline artifacts out of git" support.
//!
//! Pipeline stages write design records under `specs/vk/<task-id>/`, and
//! SpecKit files its records under `specs/<feature-key>/` (the workspace
//! branch, e.g. `specs/vk/<slug>`) beside a `.specify/` scaffold. Either can
//! sit below a repo's working subdirectory. A repo with
//! `local_pipeline_artifacts` set gets a Vibe Kanban-managed block in its
//! `info/exclude` ignoring those directories at any depth, so the records stay
//! in the worktree but never show up as untracked changes or get committed.
//! Clearing the setting removes the block. Excludes do not untrack files a
//! repo already committed.

use std::{
    io::{self, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

const BLOCK_START: &str =
    "# >>> Vibe Kanban: pipeline artifacts kept out of git (repo setting, managed)";
const BLOCK_END: &str = "# <<< Vibe Kanban: pipeline artifacts";
/// Patterns needed by every workspace, independent of its branch.
const BASE_PATTERNS: &[&str] = &["**/specs/vk/", "**/.specify/"];

/// Sidecar lock serializing updates to the shared exclude file.
const LOCK_FILE: &str = "exclude.vk-pipeline.lock";

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Add (`enabled`) or remove the managed pipeline-artifact block in the
/// `info/exclude` of the repository containing `worktree`. `feature_keys` are
/// the keys SpecKit may file this workspace's records under (its persisted
/// feature key and the current branch); the first component of each names a
/// `specs/<prefix>/` namespace. Writes only when the content changes.
///
/// In a linked worktree the exclude file lives in the shared common dir, so
/// the block applies to every worktree of the repo. Key-derived patterns
/// therefore accumulate (one per prefix) rather than being replaced by
/// whichever workspace synced last, and the read/merge/write runs under an
/// exclusive lock so concurrent setups on other nodes cannot drop each other's
/// patterns.
pub fn sync_artifact_exclude(
    worktree: &Path,
    enabled: bool,
    feature_keys: &[&str],
) -> io::Result<()> {
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
    let dir = exclude
        .parent()
        .ok_or_else(|| io::Error::other("info/exclude has no parent directory"))?;
    if !enabled && !exclude.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(dir)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(LOCK_FILE))?;
    lock.lock()?;

    let existing = match std::fs::read_to_string(&exclude) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let updated = with_managed_block(&existing, enabled.then(|| required_patterns(feature_keys)));
    if updated == existing {
        return Ok(());
    }
    // The lock is released when `lock` drops, after the replace completes.
    write_atomic(&exclude, &updated)
}

/// Patterns covering this workspace's artifacts: the task-scoped pipeline
/// directory, the SpecKit scaffold, and SpecKit's `specs/<key>` feature
/// directory for each key (by its first component, so a custom branch prefix
/// is covered too). Feature keys come from branch names, and git refnames
/// cannot contain glob characters, backslashes or spaces, so the component is
/// safe to embed.
fn required_patterns(feature_keys: &[&str]) -> Vec<String> {
    let mut patterns: Vec<String> = BASE_PATTERNS.iter().map(|p| p.to_string()).collect();
    for key in feature_keys {
        if let Some(prefix) = key.split('/').next().filter(|p| !p.is_empty()) {
            let pattern = format!("**/specs/{prefix}/");
            if !patterns.contains(&pattern) {
                patterns.push(pattern);
            }
        }
    }
    patterns
}

/// `text` with every managed block removed and, when `required` is set, one
/// block appended holding the previous block's patterns plus `required`.
fn with_managed_block(text: &str, required: Option<Vec<String>>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut previous: Vec<String> = Vec::new();
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
            } else if !trimmed.is_empty() && !previous.iter().any(|p| p == trimmed) {
                previous.push(trimmed.to_string());
            }
            continue;
        }
        out.push_str(line);
    }
    if let Some(required) = required {
        for pattern in required {
            if !previous.contains(&pattern) {
                previous.push(pattern);
            }
        }
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(BLOCK_START);
        out.push('\n');
        for pattern in previous {
            out.push_str(&pattern);
            out.push('\n');
        }
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

    fn block(patterns: &[&str]) -> String {
        let mut text = format!("{BLOCK_START}\n");
        for p in patterns {
            text.push_str(p);
            text.push('\n');
        }
        text.push_str(BLOCK_END);
        text.push('\n');
        text
    }

    #[test]
    fn block_is_added_once_and_removed_without_touching_other_lines() {
        let text = "# user\n*.log";
        let required = || Some(required_patterns(&["vk/30aa-task"]));
        let enabled = with_managed_block(text, required());
        assert_eq!(
            enabled,
            format!(
                "# user\n*.log\n{}",
                block(&["**/specs/vk/", "**/.specify/"])
            )
        );
        assert_eq!(with_managed_block(&enabled, required()), enabled);
        assert_eq!(with_managed_block(&enabled, None), "# user\n*.log\n");
        assert_eq!(with_managed_block(text, None), text);
    }

    #[test]
    fn branch_prefixes_accumulate_in_the_shared_block() {
        let first = with_managed_block("", Some(required_patterns(&["dv/feature-a"])));
        let second = with_managed_block(&first, Some(required_patterns(&["vk/task-b"])));
        assert_eq!(
            second,
            block(&["**/specs/vk/", "**/.specify/", "**/specs/dv/"])
        );
        let unprefixed = with_managed_block(&second, Some(required_patterns(&["hotfix"])));
        assert!(unprefixed.contains("**/specs/hotfix/\n"));
        assert!(unprefixed.contains("**/specs/dv/\n"));
    }

    fn write(dir: &Path, rel: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "x").unwrap();
    }

    #[test]
    fn enabled_setting_ignores_artifacts_and_disabling_restores_them() {
        let repo = init_repo();
        let artifacts = [
            "specs/vk/task-a/technical-spec.md",
            "backend/specs/vk/task-a/implementation-plan.md",
            "specs/dv/feature-a/spec.md",
            "backend/.specify/memory/constitution.md",
        ];
        for rel in artifacts {
            write(repo.path(), rel);
        }
        write(repo.path(), "specs/keep.md");
        write(repo.path(), "specs/other/keep.md");

        sync_artifact_exclude(repo.path(), true, &["dv/feature-a"]).unwrap();
        let status = untracked(repo.path());
        for rel in artifacts {
            assert!(!status.contains(rel), "{rel} not ignored: {status}");
        }
        assert!(status.contains("specs/keep.md"), "{status}");
        assert!(status.contains("specs/other/keep.md"), "{status}");

        sync_artifact_exclude(repo.path(), false, &["dv/feature-a"]).unwrap();
        let status = untracked(repo.path());
        for rel in artifacts {
            assert!(status.contains(rel), "{rel} still ignored: {status}");
        }
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

        sync_artifact_exclude(&worktree, true, &["vk/task-a"]).unwrap();

        let shared = std::fs::read_to_string(repo.path().join(".git/info/exclude")).unwrap();
        assert!(shared.contains("**/specs/vk/"));
        assert_eq!(shared.matches(BLOCK_START).count(), 1);
    }

    #[test]
    fn disabling_without_an_exclude_file_creates_nothing() {
        let repo = init_repo();
        let exclude = repo.path().join(".git/info/exclude");
        let _ = std::fs::remove_file(&exclude);

        sync_artifact_exclude(repo.path(), false, &["vk/task-a"]).unwrap();

        assert!(!exclude.exists());
    }

    #[test]
    fn persisted_feature_key_and_branch_are_both_covered() {
        let patterns = required_patterns(&["custom/task", "other/task"]);
        assert!(patterns.contains(&"**/specs/custom/".to_string()));
        assert!(patterns.contains(&"**/specs/other/".to_string()));
    }

    #[test]
    fn concurrent_syncs_keep_every_prefix() {
        let repo = init_repo();
        let path = repo.path().to_path_buf();
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let path = path.clone();
                std::thread::spawn(move || {
                    let key = format!("p{i}/task");
                    sync_artifact_exclude(&path, true, &[key.as_str()]).unwrap();
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }

        let shared = std::fs::read_to_string(path.join(".git/info/exclude")).unwrap();
        for i in 0..8 {
            assert!(shared.contains(&format!("**/specs/p{i}/\n")), "{shared}");
        }
        assert_eq!(shared.matches(BLOCK_START).count(), 1);
    }
}
