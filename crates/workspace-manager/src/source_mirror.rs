//! The courtesy mirror of a workspace branch from the shared cluster store back
//! into the operator's registered checkout (`/srv/src/<repo>`) — run once at a
//! time, off the request path, under a timeout, and backed off on failure.
//!
//! [`SharedRepositoryStore::ensure`](crate::SharedRepositoryStore::ensure) asks
//! for this mirror on every provisioning, and a clustered workspace is
//! provisioned on nearly every request that touches it. Before this module the
//! push ran inline, synchronously and without a timeout: on 2026-11-28 a 1.6 GB
//! workspace branch had 44 pushes running at once (the oldest for 77 minutes),
//! every follow-up hung behind one, and every one of them would have been
//! rejected anyway by a stale registration holding the branch.
//!
//! The rules (constitution LXI):
//!
//! - [`SourceMirror::request`] never waits on git. At most one leader task per
//!   (destination, branch) exists; a request during its flight sets a
//!   follow-up flag, so a burst collapses into at most one more push.
//! - The leader owns the slot, not the caller: a cancelled request cannot free
//!   it while git keeps running.
//! - A push that would change nothing (destination already at the store's
//!   commit) spawns nothing.
//! - Every git call has a timeout; on expiry the whole process group is killed,
//!   so `git pack-objects` dies with its parent.
//! - Failures are logged at `warn!` with git's stderr, kept for
//!   [`SourceMirror::failure`] (shown on the workspace), and backed off. A
//!   deterministic refusal is retried only once the branch has moved.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};

use chrono::{DateTime, Utc};
use tokio::{io::AsyncReadExt, sync::Notify};
use tracing::{debug, info, warn};
use utils::worktree_linkage::{self, RegistrationState};

/// Environment variable overriding [`SourceMirrorConfig::push_timeout`] (seconds).
pub const PUSH_TIMEOUT_ENV: &str = "VK_SOURCE_MIRROR_PUSH_TIMEOUT_SECS";
/// Environment variable that, when `1`/`true`, disables the mirror entirely.
pub const DISABLED_ENV: &str = "VK_SOURCE_MIRROR_DISABLED";

const DEFAULT_PUSH_TIMEOUT: Duration = Duration::from_secs(300);
const PROBE_TIMEOUT: Duration = Duration::from_secs(60);
/// How long to wait for a killed child's output pipes to close. A grandchild
/// that escaped the process group could otherwise hold them open forever.
const PIPE_DRAIN_GRACE: Duration = Duration::from_secs(5);
const STDERR_MAX_LINES: usize = 20;
const STDERR_MAX_BYTES: usize = 2048;

#[derive(Debug, Clone)]
pub struct SourceMirrorConfig {
    pub push_timeout: Duration,
    pub probe_timeout: Duration,
    pub backoff_base: Duration,
    pub transient_cap: Duration,
    pub deterministic_cap: Duration,
    pub disabled: bool,
    /// The `git` executable. `None` resolves `git` from `PATH`.
    pub git_binary: Option<PathBuf>,
}

impl Default for SourceMirrorConfig {
    fn default() -> Self {
        Self {
            push_timeout: DEFAULT_PUSH_TIMEOUT,
            probe_timeout: PROBE_TIMEOUT,
            backoff_base: Duration::from_secs(60),
            transient_cap: Duration::from_secs(30 * 60),
            deterministic_cap: Duration::from_secs(6 * 60 * 60),
            disabled: false,
            git_binary: None,
        }
    }
}

impl SourceMirrorConfig {
    pub fn from_env() -> Self {
        let mut config = Self::default();
        if let Some(secs) = std::env::var(PUSH_TIMEOUT_ENV)
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .filter(|secs| *secs > 0)
        {
            config.push_timeout = Duration::from_secs(secs);
        }
        config.disabled = std::env::var(DISABLED_ENV)
            .map(|v| matches!(v.trim(), "1" | "true" | "yes"))
            .unwrap_or(false);
        config
    }
}

/// Why the last mirror attempt failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceMirrorFailureKind {
    /// The destination refused (branch checked out there, rejected ref, not a
    /// repository…). Retrying the same commit cannot succeed.
    Deterministic,
    /// Anything else; worth retrying after a delay.
    Transient,
    /// The push did not finish within the timeout and was killed.
    Timeout,
}

/// The current failure of one (destination, branch) mirror.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMirrorFailure {
    pub kind: SourceMirrorFailureKind,
    pub message: String,
    /// Consecutive failed attempts.
    pub attempts: u32,
    pub last_attempt_at: DateTime<Utc>,
    pub next_attempt_at: DateTime<Utc>,
    /// The store commit the failure was observed at (deterministic gate).
    pub failed_tip: Option<String>,
}

/// What [`SourceMirror::request`] did. Never "succeeded": the work is async.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestOutcome {
    /// A leader task was started.
    Started,
    /// A push is already running; at most one follow-up will run after it.
    Coalesced,
    /// The last attempt failed and the backoff has not elapsed.
    BackingOff,
    /// Disabled by configuration.
    Disabled,
    /// No tokio runtime to run the leader on.
    NoRuntime,
}

type MirrorKey = (PathBuf, String);

#[derive(Debug, Default)]
struct Slot {
    in_flight: bool,
    rerun: bool,
    failure: Option<SourceMirrorFailure>,
}

struct Inner {
    config: SourceMirrorConfig,
    slots: Mutex<HashMap<MirrorKey, Slot>>,
    idle: Notify,
}

/// Process-wide registry of source-checkout mirrors. Cheap to clone.
#[derive(Clone)]
pub struct SourceMirror {
    inner: Arc<Inner>,
}

static GLOBAL: LazyLock<SourceMirror> =
    LazyLock::new(|| SourceMirror::new(SourceMirrorConfig::from_env()));

/// One round's result, as the slot sees it.
enum Round {
    /// The destination holds the store's commit (already, or after a push), or
    /// there is nothing to mirror.
    Synced { pushed: bool },
    /// Deterministic failure at an unchanged commit: nothing was spawned.
    Unchanged,
    Failed {
        kind: SourceMirrorFailureKind,
        message: String,
        tip: Option<String>,
    },
}

impl SourceMirror {
    pub fn new(config: SourceMirrorConfig) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                slots: Mutex::new(HashMap::new()),
                idle: Notify::new(),
            }),
        }
    }

    /// The instance production code uses, configured from the environment.
    pub fn global() -> &'static SourceMirror {
        &GLOBAL
    }

    /// Ask for `refs/heads/{branch}` to be mirrored from `store` into `dest`.
    /// Returns immediately; never waits on git.
    pub fn request(&self, store: &Path, dest: &Path, branch: &str) -> RequestOutcome {
        if self.inner.config.disabled {
            return RequestOutcome::Disabled;
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return RequestOutcome::NoRuntime;
        };
        let key: MirrorKey = (dest.to_path_buf(), branch.to_string());
        {
            let mut slots = self.inner.slots.lock().unwrap_or_else(|e| e.into_inner());
            let slot = slots.entry(key.clone()).or_default();
            if slot.in_flight {
                slot.rerun = true;
                return RequestOutcome::Coalesced;
            }
            if let Some(failure) = &slot.failure
                && Utc::now() < failure.next_attempt_at
            {
                return RequestOutcome::BackingOff;
            }
            slot.in_flight = true;
            slot.rerun = false;
        }
        let inner = self.inner.clone();
        let store = store.to_path_buf();
        runtime.spawn(async move { lead(inner, key, store).await });
        RequestOutcome::Started
    }

    /// The current failure for (`dest`, `branch`), if the last attempt failed.
    pub fn failure(&self, dest: &Path, branch: &str) -> Option<SourceMirrorFailure> {
        let slots = self.inner.slots.lock().unwrap_or_else(|e| e.into_inner());
        slots
            .get(&(dest.to_path_buf(), branch.to_string()))
            .and_then(|slot| slot.failure.clone())
    }

    /// Wait until no leader is running for (`dest`, `branch`).
    pub async fn wait_idle(&self, dest: &Path, branch: &str) {
        let key: MirrorKey = (dest.to_path_buf(), branch.to_string());
        loop {
            let notified = self.inner.idle.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let busy = {
                let slots = self.inner.slots.lock().unwrap_or_else(|e| e.into_inner());
                slots.get(&key).is_some_and(|slot| slot.in_flight)
            };
            if !busy {
                return;
            }
            notified.await;
        }
    }
}

/// The leader: runs rounds until one ends without a pending follow-up, then
/// releases the slot. Owns the slot for its whole life.
async fn lead(inner: Arc<Inner>, key: MirrorKey, store: PathBuf) {
    let (dest, branch) = (&key.0, &key.1);
    loop {
        let previous = {
            let slots = inner.slots.lock().unwrap_or_else(|e| e.into_inner());
            slots.get(&key).and_then(|slot| slot.failure.clone())
        };
        let round = run_round(&inner.config, &store, dest, branch, previous.as_ref()).await;

        let mut slots = inner.slots.lock().unwrap_or_else(|e| e.into_inner());
        let slot = slots.entry(key.clone()).or_default();
        let succeeded = match round {
            Round::Synced { pushed } => {
                if let Some(old) = slot.failure.take() {
                    info!(
                        dest = %dest.display(),
                        branch = %branch,
                        "source checkout mirror recovered after {} failed attempt(s)",
                        old.attempts
                    );
                } else if pushed {
                    debug!(dest = %dest.display(), branch = %branch, "mirrored branch to the source checkout");
                }
                true
            }
            Round::Unchanged => {
                if let Some(failure) = slot.failure.as_mut() {
                    let now = Utc::now();
                    failure.next_attempt_at =
                        now + backoff(&inner.config, failure.kind, failure.attempts);
                }
                false
            }
            Round::Failed { kind, message, tip } => {
                let attempts = slot.failure.as_ref().map_or(0, |f| f.attempts) + 1;
                let now = Utc::now();
                let next = now + backoff(&inner.config, kind, attempts);
                warn!(
                    dest = %dest.display(),
                    store = %store.display(),
                    branch = %branch,
                    kind = ?kind,
                    attempts,
                    next_attempt_at = %next.to_rfc3339(),
                    "could not mirror branch '{branch}' into the source checkout: {message}"
                );
                slot.failure = Some(SourceMirrorFailure {
                    kind,
                    message,
                    attempts,
                    last_attempt_at: now,
                    next_attempt_at: next,
                    failed_tip: tip,
                });
                false
            }
        };
        if succeeded && slot.rerun {
            slot.rerun = false;
            continue;
        }
        slot.in_flight = false;
        slot.rerun = false;
        drop(slots);
        inner.idle.notify_waiters();
        return;
    }
}

fn backoff(
    config: &SourceMirrorConfig,
    kind: SourceMirrorFailureKind,
    attempts: u32,
) -> chrono::Duration {
    let cap = match kind {
        SourceMirrorFailureKind::Deterministic => config.deterministic_cap,
        SourceMirrorFailureKind::Transient | SourceMirrorFailureKind::Timeout => {
            config.transient_cap
        }
    };
    let exponent = attempts.saturating_sub(1).min(20);
    let delay = config
        .backoff_base
        .checked_mul(1u32 << exponent)
        .unwrap_or(cap)
        .min(cap);
    chrono::Duration::from_std(delay).unwrap_or(chrono::Duration::MAX)
}

async fn run_round(
    config: &SourceMirrorConfig,
    store: &Path,
    dest: &Path,
    branch: &str,
    previous: Option<&SourceMirrorFailure>,
) -> Round {
    let git = config
        .git_binary
        .clone()
        .unwrap_or_else(|| PathBuf::from("git"));
    let failed = |kind, message: String, tip: Option<String>| Round::Failed { kind, message, tip };

    // 1. The store's commit. Absent means nothing to mirror.
    let store_tip = match branch_tip(&git, store, branch, config.probe_timeout).await {
        Ok(Some(tip)) => tip,
        Ok(None) => return Round::Synced { pushed: false },
        Err(e) => {
            return failed(
                SourceMirrorFailureKind::Transient,
                format!(
                    "could not read '{branch}' in the store {}: {e}",
                    store.display()
                ),
                None,
            );
        }
    };

    // 2. A deterministic refusal is not retried at the same commit.
    if let Some(previous) = previous
        && previous.kind == SourceMirrorFailureKind::Deterministic
        && previous.failed_tip.as_deref() == Some(store_tip.as_str())
    {
        return Round::Unchanged;
    }

    // 3. Nothing to do when the destination already has it.
    match branch_tip(&git, dest, branch, config.probe_timeout).await {
        Ok(Some(tip)) if tip == store_tip => return Round::Synced { pushed: false },
        Ok(_) => {}
        Err(e) => {
            let kind = classify(&e);
            return failed(
                kind,
                format!("could not read '{branch}' in {}: {e}", dest.display()),
                Some(store_tip),
            );
        }
    }

    // 4. Pre-flight: a registration holding the branch makes git refuse the
    //    push. Remove it if it is provably stale; refuse without pushing if it
    //    is live.
    if let Some(round) = preflight(&git, config, dest, branch, &store_tip).await {
        return round;
    }

    // 5. The push itself, bounded.
    let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
    let dest_arg = dest.to_string_lossy().to_string();
    match run_git_bounded(
        &git,
        store,
        &["push", &dest_arg, &refspec],
        config.push_timeout,
    )
    .await
    {
        Ok(output) if output.success => Round::Synced { pushed: true },
        Ok(output) => {
            let stderr = excerpt(&output.stderr);
            failed(
                classify(&stderr),
                format!("git push failed: {stderr}"),
                Some(store_tip),
            )
        }
        Err(BoundedError::Timeout(after)) => failed(
            SourceMirrorFailureKind::Timeout,
            format!(
                "git push did not finish within {}s and was killed with its process group",
                after.as_secs()
            ),
            Some(store_tip),
        ),
        Err(BoundedError::Spawn(e)) => failed(
            SourceMirrorFailureKind::Transient,
            format!("could not start git push: {e}"),
            Some(store_tip),
        ),
    }
}

async fn preflight(
    git: &Path,
    config: &SourceMirrorConfig,
    dest: &Path,
    branch: &str,
    store_tip: &str,
) -> Option<Round> {
    let failed = |kind, message: String| Round::Failed {
        kind,
        message,
        tip: Some(store_tip.to_string()),
    };
    let common_dir = match run_git_bounded(
        git,
        dest,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        config.probe_timeout,
    )
    .await
    {
        Ok(out) if out.success => PathBuf::from(out.stdout.trim()),
        Ok(out) => {
            let stderr = excerpt(&out.stderr);
            return Some(failed(
                classify(&stderr),
                format!("{} is not a usable repository: {stderr}", dest.display()),
            ));
        }
        Err(e) => {
            return Some(failed(
                SourceMirrorFailureKind::Transient,
                format!(
                    "could not resolve the git directory of {}: {e}",
                    dest.display()
                ),
            ));
        }
    };

    let branch = branch.to_string();
    let dest_display = dest.display().to_string();
    // Filesystem walk over possibly-NFS storage: off the async workers.
    let verdict =
        tokio::task::spawn_blocking(move || -> Result<(), (SourceMirrorFailureKind, String)> {
            let entries = worktree_linkage::registrations(&common_dir)
                .map_err(|e| (SourceMirrorFailureKind::Transient, e))?;
            for entry in entries.into_iter().flatten() {
                if entry.head_branch.as_deref() != Some(branch.as_str()) {
                    continue;
                }
                match worktree_linkage::registration_state(&entry) {
                    RegistrationState::Live => {
                        return Err((
                            SourceMirrorFailureKind::Deterministic,
                            format!(
                                "branch '{branch}' is checked out at {} in {dest_display}",
                                entry.worktree_path().display()
                            ),
                        ));
                    }
                    RegistrationState::Stale(_) => {
                        worktree_linkage::remove_stale_registration(&entry)
                            .map_err(|e| (SourceMirrorFailureKind::Transient, e))?;
                    }
                    RegistrationState::Indeterminate(reason) => {
                        return Err((
                            SourceMirrorFailureKind::Transient,
                            format!(
                                "branch '{branch}' is held by {} in {dest_display}, whose state \
                             could not be determined: {reason}",
                                entry.admin_dir.display()
                            ),
                        ));
                    }
                }
            }
            Ok(())
        })
        .await;
    match verdict {
        Ok(Ok(())) => None,
        Ok(Err((kind, message))) => Some(failed(kind, message)),
        Err(e) => Some(failed(
            SourceMirrorFailureKind::Transient,
            format!("pre-flight check panicked: {e}"),
        )),
    }
}

/// `refs/heads/{branch}` in `repo`, or `None` when the branch does not exist.
async fn branch_tip(
    git: &Path,
    repo: &Path,
    branch: &str,
    timeout: Duration,
) -> Result<Option<String>, String> {
    let spec = format!("refs/heads/{branch}^{{commit}}");
    match run_git_bounded(
        git,
        repo,
        &["rev-parse", "--verify", "--quiet", &spec],
        timeout,
    )
    .await
    {
        Ok(out) if out.success => Ok(Some(out.stdout.trim().to_string())),
        // `--verify --quiet` exits 1 with no output for a missing ref.
        Ok(out) if out.code == Some(1) && out.stderr.trim().is_empty() => Ok(None),
        Ok(out) => Err(excerpt(&out.stderr)),
        Err(e) => Err(e.to_string()),
    }
}

/// Sorts git's error text into a failure kind.
fn classify(stderr: &str) -> SourceMirrorFailureKind {
    const DETERMINISTIC: &[&str] = &[
        "remote rejected",
        "[rejected]",
        "checked out",
        "non-fast-forward",
        "fetch first",
        "does not appear to be a git repository",
        "not a git repository",
        "src refspec",
        "cannot change to",
    ];
    let lower = stderr.to_ascii_lowercase();
    if DETERMINISTIC.iter().any(|marker| lower.contains(marker)) {
        SourceMirrorFailureKind::Deterministic
    } else {
        SourceMirrorFailureKind::Transient
    }
}

/// The last lines of git's output, bounded, on one line per original line.
fn excerpt(text: &str) -> String {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let tail = &lines[lines.len().saturating_sub(STDERR_MAX_LINES)..];
    let mut joined = tail.join("\n");
    if joined.len() > STDERR_MAX_BYTES {
        let mut cut = joined.len() - STDERR_MAX_BYTES;
        while !joined.is_char_boundary(cut) {
            cut += 1;
        }
        joined = format!("…{}", &joined[cut..]);
    }
    if joined.is_empty() {
        "no output".to_string()
    } else {
        joined
    }
}

#[derive(Debug)]
struct BoundedOutput {
    success: bool,
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

#[derive(Debug)]
enum BoundedError {
    Spawn(String),
    Timeout(Duration),
}

impl std::fmt::Display for BoundedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(e) => write!(f, "could not start git: {e}"),
            Self::Timeout(after) => write!(f, "git did not finish within {}s", after.as_secs()),
        }
    }
}

/// Run `git -C <dir> -c core.hooksPath=/dev/null <args>` in its own process
/// group, killing the whole group if it outlives `timeout`.
async fn run_git_bounded(
    git: &Path,
    dir: &Path,
    args: &[&str],
    timeout: Duration,
) -> Result<BoundedOutput, BoundedError> {
    let mut cmd = tokio::process::Command::new(git);
    cmd.arg("-C")
        .arg(dir)
        // Constitution LIX: vk's own git plumbing never runs repository hooks.
        .arg("-c")
        .arg("core.hooksPath=/dev/null")
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    cmd.process_group(0);

    let mut child = cmd
        .spawn()
        .map_err(|e| BoundedError::Spawn(e.to_string()))?;
    // Saved now: `id()` is `None` once the child is reaped.
    let pid = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let read_all = |pipe: Option<tokio::process::ChildStdout>| async move {
        let mut buf = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut buf).await;
        }
        buf
    };
    let stdout_task = tokio::spawn(read_all(stdout));
    let stderr_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        if let Some(mut pipe) = stderr {
            let _ = pipe.read_to_end(&mut buf).await;
        }
        buf
    });

    let status = match tokio::time::timeout(timeout, child.wait()).await {
        Ok(status) => status.map_err(|e| BoundedError::Spawn(e.to_string()))?,
        Err(_) => {
            // The leader has not been reaped, so its pid still names our
            // group: kill all of it, then reap.
            #[cfg(unix)]
            if let Some(pid) = pid {
                utils::process::kill_process_group_now(pid);
            }
            let _ = child.kill().await;
            let _ = child.wait().await;
            stdout_task.abort();
            stderr_task.abort();
            return Err(BoundedError::Timeout(timeout));
        }
    };
    let drain = |task: tokio::task::JoinHandle<Vec<u8>>| async move {
        match tokio::time::timeout(PIPE_DRAIN_GRACE, task).await {
            Ok(Ok(buf)) => String::from_utf8_lossy(&buf).to_string(),
            _ => String::new(),
        }
    };
    Ok(BoundedOutput {
        success: status.success(),
        code: status.code(),
        stdout: drain(stdout_task).await,
        stderr: drain(stderr_task).await,
    })
}

#[cfg(test)]
mod tests {
    use std::{fs, process::Command};

    use tempfile::TempDir;

    use super::*;

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// `<root>/src` (non-bare checkout on `main`) and `<root>/store` (bare
    /// clone) where the store's `vk/b` is one commit ahead of the checkout's.
    fn fixture(root: &Path) -> (PathBuf, PathBuf) {
        let src = root.join("src");
        fs::create_dir_all(&src).unwrap();
        git(&src, &["init", "-q", "-b", "main"]);
        fs::write(src.join("a"), "a").unwrap();
        git(&src, &["add", "a"]);
        git(&src, &["commit", "-q", "-m", "a"]);
        git(&src, &["branch", "vk/b"]);
        let store = root.join("store");
        git(
            root,
            &[
                "clone",
                "-q",
                "--bare",
                &src.to_string_lossy(),
                &store.to_string_lossy(),
            ],
        );
        // Advance vk/b in the store only.
        let scratch = root.join("scratch");
        git(
            root,
            &[
                "clone",
                "-q",
                "-b",
                "vk/b",
                &store.to_string_lossy(),
                &scratch.to_string_lossy(),
            ],
        );
        fs::write(scratch.join("b"), "b").unwrap();
        git(&scratch, &["add", "b"]);
        git(&scratch, &["commit", "-q", "-m", "b"]);
        git(&scratch, &["push", "-q", "origin", "vk/b"]);
        (src, store)
    }

    fn tip(repo: &Path) -> String {
        git(repo, &["rev-parse", "refs/heads/vk/b"])
    }

    fn mirror(config: SourceMirrorConfig) -> SourceMirror {
        SourceMirror::new(config)
    }

    /// A `git` wrapper that logs every `push` to `<dir>/pushes`, flags any
    /// overlapping push in `<dir>/overlap`, runs `push_body` for pushes and
    /// delegates everything else to the real git.
    fn fake_git(dir: &Path, push_body: &str) -> PathBuf {
        let script = dir.join("fake-git");
        let real = which_git();
        let d = dir.display();
        fs::write(
            &script,
            format!(
                "#!/bin/sh\ncase \" $* \" in\n  *\" push \"*)\n    echo push >> {d}/pushes\n    mkdir {d}/lock 2>/dev/null || echo overlap >> {d}/overlap\n    {push_body}\n    rc=$?\n    rmdir {d}/lock 2>/dev/null\n    exit $rc ;;\n  *) exec {real} \"$@\" ;;\nesac\n",
                real = real.display()
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        }
        script
    }

    fn which_git() -> PathBuf {
        let out = Command::new("sh")
            .args(["-c", "command -v git"])
            .output()
            .unwrap();
        PathBuf::from(String::from_utf8_lossy(&out.stdout).trim())
    }

    fn push_count(dir: &Path) -> usize {
        fs::read_to_string(dir.join("pushes"))
            .map(|s| s.lines().count())
            .unwrap_or(0)
    }

    #[tokio::test]
    async fn a_behind_destination_is_pushed_and_an_in_sync_one_is_not() {
        let root = TempDir::new().unwrap();
        let (src, store) = fixture(root.path());
        let tools = TempDir::new().unwrap();
        let m = mirror(SourceMirrorConfig {
            git_binary: Some(fake_git(tools.path(), "exec git \"$@\"")),
            ..Default::default()
        });

        assert_eq!(m.request(&store, &src, "vk/b"), RequestOutcome::Started);
        m.wait_idle(&src, "vk/b").await;
        assert_eq!(tip(&src), tip(&store));
        assert_eq!(m.failure(&src, "vk/b"), None);
        assert_eq!(push_count(tools.path()), 1);

        // Already in sync: no push process at all.
        assert_eq!(m.request(&store, &src, "vk/b"), RequestOutcome::Started);
        m.wait_idle(&src, "vk/b").await;
        assert_eq!(
            push_count(tools.path()),
            1,
            "an in-sync branch spawns no push"
        );
    }

    #[tokio::test]
    async fn a_burst_of_requests_runs_one_push_at_a_time_and_coalesces() {
        let root = TempDir::new().unwrap();
        let (src, store) = fixture(root.path());
        let tools = TempDir::new().unwrap();
        let m = mirror(SourceMirrorConfig {
            git_binary: Some(fake_git(tools.path(), "sleep 1; git \"$@\"")),
            ..Default::default()
        });

        assert_eq!(m.request(&store, &src, "vk/b"), RequestOutcome::Started);
        for _ in 0..50 {
            assert_eq!(m.request(&store, &src, "vk/b"), RequestOutcome::Coalesced);
        }
        m.wait_idle(&src, "vk/b").await;

        assert_eq!(tip(&src), tip(&store));
        // The follow-up round finds the destination in sync and pushes nothing.
        assert_eq!(push_count(tools.path()), 1, "51 requests, one push");
        assert!(
            !tools.path().join("overlap").exists(),
            "pushes never overlap"
        );
    }

    #[tokio::test]
    async fn a_hung_push_is_killed_with_its_children_within_the_timeout() {
        let root = TempDir::new().unwrap();
        let (src, store) = fixture(root.path());
        let tools = TempDir::new().unwrap();
        let pidfile = tools.path().join("child.pid");
        let m = mirror(SourceMirrorConfig {
            push_timeout: Duration::from_secs(2),
            git_binary: Some(fake_git(
                tools.path(),
                &format!("sleep 300 & echo $! > {}; wait", pidfile.display()),
            )),
            ..Default::default()
        });

        let started = std::time::Instant::now();
        m.request(&store, &src, "vk/b");
        m.wait_idle(&src, "vk/b").await;
        assert!(
            started.elapsed() < Duration::from_secs(2 + 5),
            "{:?}",
            started.elapsed()
        );

        let failure = m.failure(&src, "vk/b").expect("a timeout is recorded");
        assert_eq!(failure.kind, SourceMirrorFailureKind::Timeout);
        assert!(failure.message.contains("killed"), "{}", failure.message);

        let child: u32 = fs::read_to_string(&pidfile)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let stat = fs::read_to_string(format!("/proc/{child}/stat"));
            let alive = stat.is_ok_and(|s| {
                s.rsplit_once(") ")
                    .is_some_and(|(_, rest)| !rest.starts_with('Z'))
            });
            if !alive {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "grandchild {child} survived the kill"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Backed off: the next request starts nothing.
        assert_eq!(m.request(&store, &src, "vk/b"), RequestOutcome::BackingOff);
    }

    #[tokio::test]
    async fn a_branch_checked_out_in_the_destination_fails_deterministically_without_a_push() {
        let root = TempDir::new().unwrap();
        let (src, store) = fixture(root.path());
        let holder = root.path().join("holder");
        git(
            &src,
            &["worktree", "add", "-q", &holder.to_string_lossy(), "vk/b"],
        );
        let tools = TempDir::new().unwrap();
        let m = mirror(SourceMirrorConfig {
            git_binary: Some(fake_git(tools.path(), "exec git \"$@\"")),
            ..Default::default()
        });

        m.request(&store, &src, "vk/b");
        m.wait_idle(&src, "vk/b").await;

        let failure = m.failure(&src, "vk/b").expect("recorded");
        assert_eq!(failure.kind, SourceMirrorFailureKind::Deterministic);
        assert!(
            failure.message.contains("checked out"),
            "{}",
            failure.message
        );
        assert_eq!(
            push_count(tools.path()),
            0,
            "a live holder is refused before pushing"
        );
        assert_eq!(m.request(&store, &src, "vk/b"), RequestOutcome::BackingOff);
    }

    #[tokio::test]
    async fn a_deterministic_failure_is_not_retried_at_the_same_commit() {
        let root = TempDir::new().unwrap();
        let (src, store) = fixture(root.path());
        let tools = TempDir::new().unwrap();
        let m = mirror(SourceMirrorConfig {
            backoff_base: Duration::ZERO,
            git_binary: Some(fake_git(
                tools.path(),
                "echo ' ! [remote rejected] vk/b -> vk/b (branch is currently checked out)' >&2; false",
            )),
            ..Default::default()
        });

        m.request(&store, &src, "vk/b");
        m.wait_idle(&src, "vk/b").await;
        let first = m.failure(&src, "vk/b").expect("recorded");
        assert_eq!(first.kind, SourceMirrorFailureKind::Deterministic);
        assert!(
            first.message.contains("remote rejected"),
            "{}",
            first.message
        );
        assert_eq!(push_count(tools.path()), 1);

        // Backoff has elapsed (zero), but the branch has not moved.
        for _ in 0..3 {
            m.request(&store, &src, "vk/b");
            m.wait_idle(&src, "vk/b").await;
        }
        assert_eq!(
            push_count(tools.path()),
            1,
            "no retry at an unchanged commit"
        );
        assert_eq!(m.failure(&src, "vk/b").unwrap().attempts, 1);
    }

    #[tokio::test]
    async fn a_stale_registration_holding_the_branch_is_removed_then_the_push_lands() {
        let root = TempDir::new().unwrap();
        let (src, store) = fixture(root.path());
        // A worktree under <root>/ws/one/repo holding vk/b, then deleted
        // without unregistering — the incident's "prunable" entry.
        let wt = root.path().join("ws").join("one").join("repo");
        fs::create_dir_all(wt.parent().unwrap()).unwrap();
        git(
            &src,
            &["worktree", "add", "-q", &wt.to_string_lossy(), "vk/b"],
        );
        fs::remove_dir_all(root.path().join("ws").join("one")).unwrap();

        let m = mirror(SourceMirrorConfig::default());
        m.request(&store, &src, "vk/b");
        m.wait_idle(&src, "vk/b").await;

        assert_eq!(m.failure(&src, "vk/b"), None);
        assert_eq!(tip(&src), tip(&store));
        let list = git(&src, &["worktree", "list", "--porcelain"]);
        assert!(!list.contains("prunable"), "{list}");
    }

    #[tokio::test]
    async fn a_disabled_mirror_does_nothing() {
        let root = TempDir::new().unwrap();
        let (src, store) = fixture(root.path());
        let before = tip(&src);
        let m = mirror(SourceMirrorConfig {
            disabled: true,
            ..Default::default()
        });
        assert_eq!(m.request(&store, &src, "vk/b"), RequestOutcome::Disabled);
        m.wait_idle(&src, "vk/b").await;
        assert_eq!(tip(&src), before);
    }

    #[test]
    fn classification_and_backoff() {
        assert_eq!(
            classify("! [remote rejected] x -> x (branch is currently checked out)"),
            SourceMirrorFailureKind::Deterministic
        );
        assert_eq!(
            classify("fatal: '/srv/src/x' does not appear to be a git repository"),
            SourceMirrorFailureKind::Deterministic
        );
        assert_eq!(
            classify("error: unable to write sha1 file"),
            SourceMirrorFailureKind::Transient
        );

        let config = SourceMirrorConfig::default();
        let t = SourceMirrorFailureKind::Transient;
        assert_eq!(backoff(&config, t, 1).num_seconds(), 60);
        assert_eq!(backoff(&config, t, 2).num_seconds(), 120);
        assert_eq!(backoff(&config, t, 50).num_seconds(), 30 * 60);
        assert_eq!(
            backoff(&config, SourceMirrorFailureKind::Deterministic, 50).num_seconds(),
            6 * 60 * 60
        );
    }

    #[test]
    fn excerpt_is_bounded_to_the_last_lines() {
        let text: String = (0..100).map(|i| format!("line {i}\n")).collect();
        let e = excerpt(&text);
        assert!(e.starts_with("line 80"), "{e}");
        assert!(e.ends_with("line 99"));
        assert!(excerpt(&"x".repeat(10_000)).len() <= STDERR_MAX_BYTES + 4);
        assert_eq!(excerpt(""), "no output");
    }
}
