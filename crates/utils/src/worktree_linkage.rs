//! Verification that a linked Git worktree resolves to the same repository on
//! every node of a cluster.
//!
//! A linked worktree is held together by two absolute paths: the worktree's
//! `.git` file names an administration directory inside the repository, and
//! that directory's `gitdir` file names the worktree's `.git` file back. When
//! the repository lives on storage only one node can see, the first pointer
//! resolves on the coordinator and dangles everywhere else — or, worse, resolves
//! to a same-named local repository holding entirely different objects. Either
//! way every Git command in the worktree fails.
//!
//! This module answers one question — *does this worktree's repository resolve
//! inside the shared root?* — using **filesystem reads only**. No `git`
//! subprocess and no `git2`, so it is callable from the worker, which depends on
//! neither. Both pointer directions are checked, because repairing one and
//! leaving the other leaves a registration that cleanup will later trip over.

use std::{
    fs,
    path::{Path, PathBuf},
};

/// What a probe found. Every variant is distinct on purpose: a failed read is
/// never reported as healthy, and "this is not a linked worktree" is not an
/// error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkageStatus {
    /// Both pointers agree and the common directory is inside the shared root.
    Portable { common_dir: PathBuf },
    /// `.git` is a directory: this is an ordinary repository, not a linked
    /// worktree. Not a defect by itself.
    OwnRepository,
    /// There is no `.git` entry at all.
    Missing,
    /// `.git` names an administration directory that does not exist.
    Dangling { target: PathBuf },
    /// The pointers resolve, but to a repository outside the shared root — so
    /// another node cannot reach it.
    OutsideSharedRoot { common_dir: PathBuf },
    /// The administration directory exists but points back at a different
    /// worktree. Repairing one direction and not the other produces this.
    BackPointerMismatch { expected: PathBuf, found: PathBuf },
    /// A read or stat failed, or `.git` could not be parsed. Never collapse this
    /// into `Portable`: on network storage a transient failure is routine, and
    /// treating "could not tell" as "fine" is how work gets destroyed.
    Indeterminate { reason: String },
}

impl LinkageStatus {
    /// True only for `Portable`. Deliberately not `!is_broken()`.
    pub fn is_portable(&self) -> bool {
        matches!(self, Self::Portable { .. })
    }

    /// True when the worktree is definitely unusable from another node.
    /// `Indeterminate` is **not** included — it is unknown, not broken.
    pub fn is_broken(&self) -> bool {
        matches!(
            self,
            Self::Dangling { .. }
                | Self::OutsideSharedRoot { .. }
                | Self::BackPointerMismatch { .. }
        )
    }

    /// A one-line operator-readable explanation.
    pub fn describe(&self) -> String {
        match self {
            Self::Portable { common_dir } => {
                format!("resolves to {}", common_dir.display())
            }
            Self::OwnRepository => "is an ordinary repository, not a linked worktree".to_string(),
            Self::Missing => "has no .git entry".to_string(),
            Self::Dangling { target } => {
                format!("points at {}, which does not exist", target.display())
            }
            Self::OutsideSharedRoot { common_dir } => format!(
                "resolves to {}, which is outside the shared root",
                common_dir.display()
            ),
            Self::BackPointerMismatch { expected, found } => format!(
                "administration directory points back at {} instead of {}",
                found.display(),
                expected.display()
            ),
            Self::Indeterminate { reason } => format!("could not be determined: {reason}"),
        }
    }
}

/// Reads and interprets a worktree's Git linkage.
#[derive(Debug, Clone)]
pub struct WorktreeLinkage;

impl WorktreeLinkage {
    /// Probe `worktree_path` and report whether its repository resolves inside
    /// `shared_root`.
    ///
    /// Filesystem reads only. Every "does this exist" question goes through
    /// [`fs::exists`] rather than `Path::exists`, because the latter reports
    /// `false` for both "absent" and "stat failed".
    pub fn probe(worktree_path: &Path, shared_root: &Path) -> LinkageStatus {
        let git_entry = worktree_path.join(".git");

        let metadata = match fs::symlink_metadata(&git_entry) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return LinkageStatus::Missing,
            Err(e) => {
                return indeterminate(format_args!("stat {}: {e}", git_entry.display()));
            }
        };

        if metadata.is_dir() {
            return LinkageStatus::OwnRepository;
        }

        let contents = match fs::read_to_string(&git_entry) {
            Ok(contents) => contents,
            Err(e) => return indeterminate(format_args!("read {}: {e}", git_entry.display())),
        };

        // Refuse to guess at an unparseable pointer rather than inventing one.
        let Some(admin_dir) = parse_gitdir_pointer(&contents, worktree_path) else {
            return indeterminate(format_args!(
                "{} is not a `gitdir:` pointer",
                git_entry.display()
            ));
        };

        match fs::exists(&admin_dir) {
            Ok(true) => {}
            Ok(false) => return LinkageStatus::Dangling { target: admin_dir },
            Err(e) => {
                return indeterminate(format_args!("stat {}: {e}", admin_dir.display()));
            }
        }

        // Direction two: the administration directory must name this worktree.
        // Checking only direction one leaves a dangling registration behind.
        let back_pointer_file = admin_dir.join("gitdir");
        match fs::read_to_string(&back_pointer_file) {
            Ok(back) => {
                let found = PathBuf::from(back.trim());
                if !same_path(&found, &git_entry) {
                    return LinkageStatus::BackPointerMismatch {
                        expected: git_entry,
                        found,
                    };
                }
            }
            Err(e) => {
                return indeterminate(format_args!("read {}: {e}", back_pointer_file.display()));
            }
        }

        let common_dir = match resolve_common_dir(&admin_dir) {
            Ok(dir) => dir,
            Err(reason) => return LinkageStatus::Indeterminate { reason },
        };

        // Structural containment, never a substring test against a known-bad
        // prefix: the question is "is this reachable at the same path on every
        // node", and only the shared root answers it.
        let canonical_root = match fs::canonicalize(shared_root) {
            Ok(root) => root,
            Err(e) => {
                return indeterminate(format_args!("canonicalize {}: {e}", shared_root.display()));
            }
        };

        if common_dir.starts_with(&canonical_root) {
            LinkageStatus::Portable { common_dir }
        } else {
            LinkageStatus::OutsideSharedRoot { common_dir }
        }
    }
}

/// One entry under a repository's `<common_dir>/worktrees/`: what Git calls a
/// worktree *registration*. Read from the filesystem only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    /// `<common_dir>/worktrees/<name>`.
    pub admin_dir: PathBuf,
    /// The worktree's `.git` file, as recorded in the entry's `gitdir` file.
    pub worktree_git: PathBuf,
    /// The branch the entry holds checked out (`HEAD` is `ref: refs/heads/…`),
    /// or `None` for a detached HEAD.
    pub head_branch: Option<String>,
    /// Whether the entry carries a `locked` file (`git worktree lock`).
    pub locked: bool,
}

impl Registration {
    /// The worktree directory this entry claims.
    pub fn worktree_path(&self) -> &Path {
        self.worktree_git.parent().unwrap_or(&self.worktree_git)
    }
}

/// Whether a [`Registration`] is safe to remove.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationState {
    /// The worktree exists and points back at this entry, or the entry is
    /// locked. Never removed.
    Live,
    /// Positive evidence that nothing uses this entry any more: the worktree
    /// it names now points at a *different*, existing registration (it was
    /// re-linked elsewhere). Removable as is. The reason is operator-readable.
    Stale(String),
    /// The worktree directory definitely does not exist. That alone does not
    /// say whether it was deleted or its storage is offline (a missing mount
    /// looks exactly like this), so it is removable only with evidence about
    /// that storage from the caller — see
    /// [`remove_registration_of_deleted_worktree`].
    WorktreeMissing(String),
    /// Could not tell. Treated exactly like `Live`: an error is never evidence
    /// that a registration is unused.
    Indeterminate(String),
}

/// Lists the worktree registrations of the repository whose common Git
/// directory is `common_dir` (for a non-bare checkout, its `.git` directory).
///
/// A repository without a `worktrees/` directory has no registrations. Each
/// entry that cannot be read comes back as an `Err` naming it, never silently
/// dropped, so a caller deciding what to delete can see what it did not
/// inspect.
pub fn registrations(common_dir: &Path) -> Result<Vec<Result<Registration, String>>, String> {
    let worktrees = common_dir.join("worktrees");
    let entries = match fs::read_dir(&worktrees) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("read {}: {e}", worktrees.display())),
    };
    let mut out = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => out.push(read_registration(&entry.path())),
            Err(e) => out.push(Err(format!("read entry of {}: {e}", worktrees.display()))),
        }
    }
    Ok(out)
}

/// The administration directory a worktree's `.git` file currently names, or
/// `None` when it cannot be read or parsed.
pub fn gitdir_target(worktree_path: &Path) -> Option<PathBuf> {
    let contents = fs::read_to_string(worktree_path.join(".git")).ok()?;
    parse_gitdir_pointer(&contents, worktree_path)
}

/// Resolves `.` and `..` components without touching the filesystem (the path
/// may name something that no longer exists, so `canonicalize` cannot help).
fn normalize_lexically(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(component);
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// Reads one registration directory (`<common_dir>/worktrees/<name>`).
pub fn read_registration(admin_dir: &Path) -> Result<Registration, String> {
    let read = |name: &str| {
        let path = admin_dir.join(name);
        fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))
    };
    let gitdir = read("gitdir")?;
    let gitdir = gitdir.trim();
    if gitdir.is_empty() {
        return Err(format!("{} has an empty gitdir", admin_dir.display()));
    }
    // Relative when git writes them so (`worktree.useRelativePaths`):
    // relative to the admin directory, and normalised lexically so it compares
    // equal to the absolute path callers hold, even once the worktree is gone.
    let worktree_git = {
        let p = Path::new(gitdir);
        if p.is_absolute() {
            normalize_lexically(p)
        } else {
            normalize_lexically(&admin_dir.join(p))
        }
    };
    let head = read("HEAD")?;
    let head_branch = head
        .trim()
        .strip_prefix("ref:")
        .map(str::trim)
        .and_then(|r| r.strip_prefix("refs/heads/"))
        .map(str::to_string);
    let locked = admin_dir
        .join("locked")
        .try_exists()
        .map_err(|e| format!("stat {}/locked: {e}", admin_dir.display()))?;
    Ok(Registration {
        admin_dir: admin_dir.to_path_buf(),
        worktree_git,
        head_branch,
        locked,
    })
}

/// Decides whether `registration` is still in use, from what this entry and its
/// worktree path show. Locked means [`RegistrationState::Live`]. A worktree
/// re-linked to another existing registration means
/// [`RegistrationState::Stale`]. A definitely absent worktree directory means
/// [`RegistrationState::WorktreeMissing`] — never `Stale`, because only the
/// caller can know whether the storage that held it is mounted. Every read or
/// stat failure is [`RegistrationState::Indeterminate`].
pub fn registration_state(registration: &Registration) -> RegistrationState {
    if registration.locked {
        return RegistrationState::Live;
    }
    let git_entry = &registration.worktree_git;
    let worktree = registration.worktree_path();
    match fs::symlink_metadata(git_entry) {
        Ok(meta) if meta.is_dir() => RegistrationState::Indeterminate(format!(
            "{} is a directory, not a worktree pointer",
            git_entry.display()
        )),
        Ok(_) => {
            let contents = match fs::read_to_string(git_entry) {
                Ok(c) => c,
                Err(e) => {
                    return RegistrationState::Indeterminate(format!(
                        "read {}: {e}",
                        git_entry.display()
                    ));
                }
            };
            let Some(target) = parse_gitdir_pointer(&contents, worktree) else {
                return RegistrationState::Indeterminate(format!(
                    "{} is not a `gitdir:` pointer",
                    git_entry.display()
                ));
            };
            if same_path(&target, &registration.admin_dir) {
                return RegistrationState::Live;
            }
            match target.try_exists() {
                // Positive evidence of re-linking: the replacement is itself a
                // registration whose `gitdir` names this very worktree. A
                // pointer to anything else (a partial repair, a bad write) is
                // not, and this entry may still be the only valid one.
                Ok(true) => match fs::read_to_string(target.join("gitdir")) {
                    Ok(back) if same_path(Path::new(back.trim()), git_entry) => {
                        RegistrationState::Stale(format!(
                            "{} was re-linked to {}",
                            worktree.display(),
                            target.display()
                        ))
                    }
                    Ok(back) => RegistrationState::Indeterminate(format!(
                        "{} points at {}, which names {} rather than this worktree",
                        git_entry.display(),
                        target.display(),
                        back.trim()
                    )),
                    Err(e) => RegistrationState::Indeterminate(format!(
                        "{} points at {}, which is not a registration: {e}",
                        git_entry.display(),
                        target.display()
                    )),
                },
                Ok(false) => RegistrationState::Indeterminate(format!(
                    "{} points at {}, which does not exist",
                    git_entry.display(),
                    target.display()
                )),
                Err(e) => {
                    RegistrationState::Indeterminate(format!("stat {}: {e}", target.display()))
                }
            }
        }
        // `symlink_metadata`, not `try_exists`: a worktree path that is a
        // symlink into an offline volume dangles — that is not a deletion.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => match fs::symlink_metadata(worktree)
            .map(|_| true)
            .or_else(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Ok(false)
                } else {
                    Err(e)
                }
            }) {
            Ok(false) => RegistrationState::WorktreeMissing(format!(
                "worktree directory {} does not exist",
                worktree.display()
            )),
            Ok(true) => RegistrationState::Indeterminate(format!(
                "{} exists but has no .git entry",
                worktree.display()
            )),
            Err(e) => RegistrationState::Indeterminate(format!("stat {}: {e}", worktree.display())),
        },
        Err(e) => RegistrationState::Indeterminate(format!("stat {}: {e}", git_entry.display())),
    }
}

/// Moves a [`RegistrationState::Stale`] (re-linked) entry **aside** rather
/// than deleting it: out of `<common>/worktrees/`, so git no longer counts it
/// (it stops holding its branch), into `<common>/vk-retired-worktrees/
/// <name>.<epoch>`. Used after adoption, where the entry's `index` may be the
/// only record of staged work: the worktree's index was rebuilt from `HEAD`,
/// so a staged version that was edited again lives nowhere else. Returns the
/// new location, or `None` when the entry is not (or no longer) stale.
pub fn retire_stale_registration(registration: &Registration) -> Result<Option<PathBuf>, String> {
    let current = match read_registration(&registration.admin_dir) {
        Ok(current) => current,
        Err(_) => return Ok(None),
    };
    if current != *registration {
        return Ok(None);
    }
    let RegistrationState::Stale(reason) = registration_state(&current) else {
        return Ok(None);
    };
    let (Some(name), Some(common_dir)) = (
        current.admin_dir.file_name(),
        current.admin_dir.parent().and_then(Path::parent),
    ) else {
        return Err(format!(
            "{} is not under <common>/worktrees",
            current.admin_dir.display()
        ));
    };
    let retired_root = common_dir.join("vk-retired-worktrees");
    fs::create_dir_all(&retired_root)
        .map_err(|e| format!("create {}: {e}", retired_root.display()))?;
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let target = retired_root.join(format!("{}.{epoch}", name.to_string_lossy()));
    tracing::info!(
        admin_dir = %current.admin_dir.display(),
        retired_to = %target.display(),
        branch = current.head_branch.as_deref().unwrap_or("-"),
        "retiring stale worktree registration (kept for its index): {reason}"
    );
    fs::rename(&current.admin_dir, &target)
        .map(|()| Some(target))
        .map_err(|e| format!("move {} aside: {e}", current.admin_dir.display()))
}

/// Removes a [`RegistrationState::WorktreeMissing`] entry on the caller's
/// evidence that the worktree was really deleted rather than made invisible by
/// offline storage: the caller just deleted it, or the storage it lived on is
/// verifiably present (e.g. it visibly holds a live workspace). Still re-reads
/// the entry and refuses if it changed, is locked, or its path exists again.
pub fn remove_registration_of_deleted_worktree(
    registration: &Registration,
) -> Result<bool, String> {
    let current = match read_registration(&registration.admin_dir) {
        Ok(current) => current,
        Err(_) => return Ok(false),
    };
    if current != *registration || current.locked {
        return Ok(false);
    }
    // `symlink_metadata`: a path recreated as a symlink (perhaps into an
    // offline volume) has reappeared, whatever it points at.
    if !matches!(
        fs::symlink_metadata(current.worktree_path()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound
    ) {
        return Ok(false);
    }
    tracing::info!(
        admin_dir = %current.admin_dir.display(),
        branch = current.head_branch.as_deref().unwrap_or("-"),
        "removing the registration of deleted worktree {}",
        current.worktree_path().display()
    );
    fs::remove_dir_all(&current.admin_dir)
        .map(|()| true)
        .map_err(|e| format!("remove {}: {e}", current.admin_dir.display()))
}

/// `refs/heads/{branch}` read straight from a common Git directory: the loose
/// ref file, else `packed-refs`. `Ok(None)`: the branch does not exist.
/// `Err`: cannot tell without git (a symbolic ref, the reftable backend, an
/// unreadable file) — callers fall back to `git rev-parse`. No subprocess.
pub fn read_branch_tip(common_dir: &Path, branch: &str) -> Result<Option<String>, String> {
    let is_oid =
        |s: &str| (s.len() == 40 || s.len() == 64) && s.bytes().all(|b| b.is_ascii_hexdigit());
    if common_dir.join("reftable").exists() {
        return Err("reftable repository".to_string());
    }
    let refname = format!("refs/heads/{branch}");
    let loose = common_dir.join(&refname);
    match fs::read_to_string(&loose) {
        Ok(contents) => {
            let oid = contents.trim();
            return if is_oid(oid) {
                Ok(Some(oid.to_string()))
            } else {
                Err(format!("{} is not a plain object id", loose.display()))
            };
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        // A directory (refs/heads/vk is a namespace) means no such branch.
        Err(_) if loose.is_dir() => return Ok(None),
        Err(e) => return Err(format!("read {}: {e}", loose.display())),
    }
    let packed = common_dir.join("packed-refs");
    let contents = match fs::read_to_string(&packed) {
        Ok(contents) => contents,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("read {}: {e}", packed.display())),
    };
    for line in contents.lines() {
        if let Some((oid, name)) = line.split_once(' ')
            && name == refname
            && is_oid(oid)
        {
            return Ok(Some(oid.to_string()));
        }
    }
    Ok(None)
}

/// Where a repository's shared Git directory is, read from the filesystem
/// only (no `git` subprocess, so it cannot hang on a child process), and
/// whether the repository is bare.
///
/// - `<repo>/.git` is a directory: a normal checkout; that directory.
/// - `<repo>/.git` is a `gitdir:` file: a linked worktree or submodule; its
///   administration directory's `commondir`.
/// - no `.git`, but `<repo>/HEAD` and `<repo>/objects`: a bare repository.
pub fn common_dir_of(repo: &Path) -> Result<(PathBuf, bool), String> {
    let dot_git = repo.join(".git");
    // `metadata`, not `symlink_metadata`: git follows a `.git` symlink to a
    // directory, and so must we.
    match fs::metadata(&dot_git) {
        Ok(meta) if meta.is_dir() => Ok((dot_git, false)),
        Ok(_) => {
            let contents = fs::read_to_string(&dot_git)
                .map_err(|e| format!("read {}: {e}", dot_git.display()))?;
            let admin = parse_gitdir_pointer(&contents, repo)
                .ok_or_else(|| format!("{} is not a `gitdir:` pointer", dot_git.display()))?;
            match resolve_common_dir(&admin) {
                Ok(common) => Ok((common, false)),
                // A submodule's gitdir has no `commondir`: it is its own.
                Err(_) if admin.join("HEAD").exists() => Ok((admin, false)),
                Err(e) => Err(e),
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if repo.join("HEAD").is_file() && repo.join("objects").is_dir() {
                Ok((repo.to_path_buf(), true))
            } else {
                Err(format!("{} is not a git repository", repo.display()))
            }
        }
        Err(e) => Err(format!("stat {}: {e}", dot_git.display())),
    }
}

/// Removes `registration`'s administration directory if, re-checked right now,
/// it is [`RegistrationState::Stale`]. Returns whether it was removed. Logs the
/// entry and the reason at `info!` before acting. Only Git's bookkeeping
/// directory is removed; no worktree file is ever touched.
pub fn remove_stale_registration(registration: &Registration) -> Result<bool, String> {
    // Re-read the entry rather than trusting the caller's copy: git may have
    // replaced it (same admin path, new worktree) or locked it since it was
    // listed. A changed or unreadable entry is not the one judged stale.
    let current = match read_registration(&registration.admin_dir) {
        Ok(current) => current,
        Err(_) => return Ok(false),
    };
    if current != *registration {
        return Ok(false);
    }
    let RegistrationState::Stale(reason) = registration_state(&current) else {
        return Ok(false);
    };
    tracing::info!(
        admin_dir = %registration.admin_dir.display(),
        branch = registration.head_branch.as_deref().unwrap_or("-"),
        "removing stale worktree registration: {reason}"
    );
    fs::remove_dir_all(&registration.admin_dir)
        .map(|()| true)
        .map_err(|e| format!("remove {}: {e}", registration.admin_dir.display()))
}

fn indeterminate(reason: std::fmt::Arguments<'_>) -> LinkageStatus {
    LinkageStatus::Indeterminate {
        reason: reason.to_string(),
    }
}

/// Parses the single `gitdir: <path>` line Git writes into a linked worktree's
/// `.git` file. A relative pointer is resolved against the worktree, which is
/// how Git itself interprets it.
fn parse_gitdir_pointer(contents: &str, worktree_path: &Path) -> Option<PathBuf> {
    let target = contents
        .lines()
        .find_map(|line| line.trim().strip_prefix("gitdir:"))?
        .trim();
    if target.is_empty() {
        return None;
    }
    let target = Path::new(target);
    Some(if target.is_absolute() {
        target.to_path_buf()
    } else {
        worktree_path.join(target)
    })
}

/// Resolves `<admin_dir>/commondir` — the repository directory the worktree
/// shares with its siblings. Its absence means the administration directory is
/// not a worktree registration, which we report as unknown rather than guessing.
fn resolve_common_dir(admin_dir: &Path) -> Result<PathBuf, String> {
    let common_dir_file = admin_dir.join("commondir");
    let raw = fs::read_to_string(&common_dir_file)
        .map_err(|e| format!("read {}: {e}", common_dir_file.display()))?;
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(format!("{} is empty", common_dir_file.display()));
    }
    let candidate = Path::new(raw);
    let joined = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        admin_dir.join(candidate)
    };
    fs::canonicalize(&joined).map_err(|e| format!("canonicalize {}: {e}", joined.display()))
}

/// Compares two paths for identity, falling back to a lexical comparison when
/// either cannot be canonicalised (it may legitimately not exist yet).
fn same_path(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    /// `<root>/src/.git` with one registration `wt` for `<root>/ws/one/repo`
    /// holding `vk/b`. Returns (common_dir, admin_dir, worktree).
    fn source_registration(root: &Path) -> (PathBuf, PathBuf, PathBuf) {
        let common = root.join("src").join(".git");
        let admin = common.join("worktrees").join("wt");
        let worktree = root.join("ws").join("one").join("repo");
        fs::create_dir_all(&admin).unwrap();
        fs::create_dir_all(&worktree).unwrap();
        fs::write(
            admin.join("gitdir"),
            format!("{}\n", worktree.join(".git").display()),
        )
        .unwrap();
        fs::write(admin.join("HEAD"), "ref: refs/heads/vk/b\n").unwrap();
        fs::write(admin.join("commondir"), "../..\n").unwrap();
        fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", admin.display()),
        )
        .unwrap();
        // Another workspace on the same storage: evidence that it is mounted.
        let sibling = root.join("ws").join("sibling").join("repo");
        fs::create_dir_all(&sibling).unwrap();
        fs::write(sibling.join(".git"), "gitdir: /elsewhere\n").unwrap();
        (common, admin, worktree)
    }

    fn only(common: &Path) -> Registration {
        let mut regs = registrations(common).unwrap();
        assert_eq!(regs.len(), 1);
        regs.remove(0).unwrap()
    }

    #[test]
    fn a_live_registration_is_kept_and_reports_its_branch() {
        let root = TempDir::new().unwrap();
        let (common, admin, worktree) = source_registration(root.path());
        let reg = only(&common);
        assert_eq!(reg.head_branch.as_deref(), Some("vk/b"));
        assert_eq!(reg.worktree_path(), worktree.as_path());
        assert_eq!(registration_state(&reg), RegistrationState::Live);
        assert!(!remove_stale_registration(&reg).unwrap());
        assert!(admin.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_git_directory_is_followed() {
        let root = TempDir::new().unwrap();
        let real = root.path().join("real-git");
        fs::create_dir_all(&real).unwrap();
        let checkout = root.path().join("checkout");
        fs::create_dir_all(&checkout).unwrap();
        std::os::unix::fs::symlink(&real, checkout.join(".git")).unwrap();
        assert_eq!(
            common_dir_of(&checkout).unwrap(),
            (checkout.join(".git"), false)
        );
    }

    #[test]
    fn a_missing_worktree_is_never_stale_on_its_own() {
        let root = TempDir::new().unwrap();
        let (common, admin, _) = source_registration(root.path());
        // Deleted, or behind an offline mount: indistinguishable from here.
        fs::remove_dir_all(root.path().join("ws").join("one")).unwrap();
        let reg = only(&common);
        assert!(matches!(
            registration_state(&reg),
            RegistrationState::WorktreeMissing(_)
        ));
        assert!(!remove_stale_registration(&reg).unwrap());
        assert!(admin.exists());
    }

    #[test]
    fn a_just_deleted_last_worktree_is_removed_on_the_callers_evidence() {
        let root = TempDir::new().unwrap();
        let (common, admin, _) = source_registration(root.path());
        // The last worktree on its storage: no sibling to prove the mount.
        fs::remove_dir_all(root.path().join("ws")).unwrap();
        let listed = only(&common);
        assert!(matches!(
            registration_state(&listed),
            RegistrationState::WorktreeMissing(_)
        ));
        assert!(remove_registration_of_deleted_worktree(&listed).unwrap());
        assert!(!admin.exists());
    }

    #[test]
    fn deletion_evidence_does_not_override_a_lock_or_a_recreated_worktree() {
        let root = TempDir::new().unwrap();
        let (common, admin, worktree) = source_registration(root.path());
        let listed = only(&common);
        // Still there: no evidence of deletion.
        assert!(!remove_registration_of_deleted_worktree(&listed).unwrap());
        fs::remove_dir_all(&worktree).unwrap();
        fs::write(admin.join("locked"), "").unwrap();
        assert!(!remove_registration_of_deleted_worktree(&listed).unwrap());
        assert!(admin.exists());
    }

    #[test]
    fn a_relative_gitdir_is_normalised_against_the_admin_directory() {
        let root = TempDir::new().unwrap();
        let (common, admin, worktree) = source_registration(root.path());
        // What `worktree.useRelativePaths` writes: relative to the admin dir.
        fs::write(admin.join("gitdir"), "../../../../ws/one/repo/.git\n").unwrap();
        let reg = only(&common);
        assert_eq!(reg.worktree_path(), worktree.as_path());
        assert_eq!(registration_state(&reg), RegistrationState::Live);
    }

    #[test]
    fn a_retired_registration_keeps_its_index_outside_worktrees() {
        let root = TempDir::new().unwrap();
        let (common, admin, worktree) = source_registration(root.path());
        fs::write(admin.join("index"), "staged state").unwrap();
        let other = root.path().join("store").join("worktrees").join("wt");
        fs::create_dir_all(&other).unwrap();
        // A real replacement registration, pointing back at this worktree.
        fs::write(
            other.join("gitdir"),
            format!("{}\n", worktree.join(".git").display()),
        )
        .unwrap();
        fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", other.display()),
        )
        .unwrap();

        let retired = retire_stale_registration(&only(&common)).unwrap().unwrap();
        assert!(!admin.exists(), "no longer a registration");
        assert!(registrations(&common).unwrap().is_empty());
        assert_eq!(
            fs::read_to_string(retired.join("index")).unwrap(),
            "staged state"
        );
        assert!(retired.starts_with(common.join("vk-retired-worktrees")));
    }

    #[test]
    fn branch_tips_are_read_from_loose_and_packed_refs() {
        let root = TempDir::new().unwrap();
        let common = root.path();
        let a = "a".repeat(40);
        let b = "b".repeat(40);
        fs::create_dir_all(common.join("refs/heads/vk")).unwrap();
        fs::write(common.join("refs/heads/vk/loose"), format!("{a}\n")).unwrap();
        fs::write(
            common.join("packed-refs"),
            format!("# pack-refs with: peeled fully-peeled sorted\n{b} refs/heads/vk/packed\n"),
        )
        .unwrap();
        assert_eq!(read_branch_tip(common, "vk/loose").unwrap(), Some(a));
        assert_eq!(read_branch_tip(common, "vk/packed").unwrap(), Some(b));
        assert_eq!(read_branch_tip(common, "vk/missing").unwrap(), None);
        assert_eq!(read_branch_tip(common, "vk").unwrap(), None);
        fs::write(common.join("refs/heads/sym"), "ref: refs/heads/vk/loose\n").unwrap();
        assert!(read_branch_tip(common, "sym").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_symlinked_worktree_is_not_missing() {
        let root = TempDir::new().unwrap();
        let (common, _, worktree) = source_registration(root.path());
        fs::remove_dir_all(&worktree).unwrap();
        std::os::unix::fs::symlink(root.path().join("offline-volume/repo"), &worktree).unwrap();
        assert!(matches!(
            registration_state(&only(&common)),
            RegistrationState::Indeterminate(_)
        ));
    }

    #[test]
    fn common_dir_is_resolved_without_git() {
        let root = TempDir::new().unwrap();
        let (common, _, worktree) = source_registration(root.path());
        let checkout = common.parent().unwrap();
        assert_eq!(common_dir_of(checkout).unwrap(), (common.clone(), false));
        assert_eq!(
            common_dir_of(&worktree).unwrap(),
            (fs::canonicalize(&common).unwrap(), false)
        );
        let bare = root.path().join("bare");
        fs::create_dir_all(bare.join("objects")).unwrap();
        fs::write(bare.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        assert_eq!(common_dir_of(&bare).unwrap(), (bare.clone(), true));
        assert!(common_dir_of(&root.path().join("nothing")).is_err());
    }

    #[test]
    fn a_locked_registration_is_live_even_when_its_worktree_is_gone() {
        let root = TempDir::new().unwrap();
        let (common, admin, _) = source_registration(root.path());
        fs::write(admin.join("locked"), "on a usb stick\n").unwrap();
        fs::remove_dir_all(root.path().join("ws").join("one")).unwrap();
        let reg = only(&common);
        assert!(reg.locked);
        assert_eq!(registration_state(&reg), RegistrationState::Live);
    }

    #[test]
    fn a_worktree_relinked_elsewhere_leaves_this_entry_stale() {
        let root = TempDir::new().unwrap();
        let (common, admin, worktree) = source_registration(root.path());
        let other = root.path().join("store").join("worktrees").join("wt");
        fs::create_dir_all(&other).unwrap();
        // A real replacement registration, pointing back at this worktree.
        fs::write(
            other.join("gitdir"),
            format!("{}\n", worktree.join(".git").display()),
        )
        .unwrap();
        fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", other.display()),
        )
        .unwrap();
        let reg = only(&common);
        assert!(matches!(
            registration_state(&reg),
            RegistrationState::Stale(_)
        ));
        assert!(remove_stale_registration(&reg).unwrap());
        assert!(!admin.exists());
        assert!(
            worktree.join(".git").exists(),
            "the worktree is never touched"
        );
    }

    #[test]
    fn an_entry_replaced_or_locked_after_listing_is_not_removed() {
        let root = TempDir::new().unwrap();
        let (common, admin, worktree) = source_registration(root.path());
        let other = root.path().join("store").join("worktrees").join("wt");
        fs::create_dir_all(&other).unwrap();
        // A real replacement registration, pointing back at this worktree.
        fs::write(
            other.join("gitdir"),
            format!("{}\n", worktree.join(".git").display()),
        )
        .unwrap();
        fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", other.display()),
        )
        .unwrap();
        let listed = only(&common);
        assert!(matches!(
            registration_state(&listed),
            RegistrationState::Stale(_)
        ));

        // Git reuses the admin path for a new, live worktree.
        let fresh = root.path().join("ws").join("two").join("repo");
        fs::create_dir_all(&fresh).unwrap();
        fs::write(
            admin.join("gitdir"),
            format!("{}\n", fresh.join(".git").display()),
        )
        .unwrap();
        fs::write(fresh.join(".git"), format!("gitdir: {}\n", admin.display())).unwrap();
        assert!(!remove_stale_registration(&listed).unwrap());
        assert!(admin.exists());

        // Or the original entry gets locked.
        fs::write(
            admin.join("gitdir"),
            format!("{}\n", worktree.join(".git").display()),
        )
        .unwrap();
        let listed = only(&common);
        fs::write(admin.join("locked"), "").unwrap();
        assert!(!remove_stale_registration(&listed).unwrap());
        assert!(admin.exists());
    }

    #[test]
    fn a_pointer_to_something_that_is_not_its_registration_is_indeterminate() {
        let root = TempDir::new().unwrap();
        let (common, _, worktree) = source_registration(root.path());
        // Points at an existing directory that names a different worktree.
        let other = root.path().join("store").join("worktrees").join("other");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("gitdir"), "/somewhere/else/.git\n").unwrap();
        fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", other.display()),
        )
        .unwrap();
        assert!(matches!(
            registration_state(&only(&common)),
            RegistrationState::Indeterminate(_)
        ));
    }

    #[test]
    fn a_pointer_to_a_missing_admin_dir_is_indeterminate() {
        let root = TempDir::new().unwrap();
        let (common, _, worktree) = source_registration(root.path());
        fs::write(worktree.join(".git"), "gitdir: /nonexistent/admin\n").unwrap();
        assert!(matches!(
            registration_state(&only(&common)),
            RegistrationState::Indeterminate(_)
        ));
    }

    #[test]
    fn an_unreadable_entry_is_reported_not_dropped() {
        let root = TempDir::new().unwrap();
        let (common, admin, _) = source_registration(root.path());
        fs::remove_file(admin.join("gitdir")).unwrap();
        let regs = registrations(&common).unwrap();
        assert_eq!(regs.len(), 1);
        assert!(regs[0].is_err());
    }

    #[test]
    fn a_repository_without_worktrees_has_no_registrations() {
        let root = TempDir::new().unwrap();
        assert!(registrations(root.path()).unwrap().is_empty());
    }

    /// Builds `<root>/store` as a stand-in bare repository with one worktree
    /// registration, and `<root>/ws/repo` as the worktree pointing at it.
    fn linked_pair(root: &Path, admin_name: &str) -> (PathBuf, PathBuf) {
        let store = root.join("store");
        let admin = store.join("worktrees").join(admin_name);
        let worktree = root.join("ws").join("repo");
        fs::create_dir_all(&admin).unwrap();
        fs::create_dir_all(&worktree).unwrap();
        fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", admin.display()),
        )
        .unwrap();
        fs::write(
            admin.join("gitdir"),
            format!("{}\n", worktree.join(".git").display()),
        )
        .unwrap();
        fs::write(admin.join("commondir"), "../..\n").unwrap();
        (store, worktree)
    }

    #[test]
    fn reports_portable_when_both_pointers_agree_inside_the_shared_root() {
        let fixture = TempDir::new().unwrap();
        let root = fixture.path();
        let (store, worktree) = linked_pair(root, "repo");

        let status = WorktreeLinkage::probe(&worktree, root);

        assert_eq!(
            status,
            LinkageStatus::Portable {
                common_dir: fs::canonicalize(&store).unwrap()
            }
        );
        assert!(status.is_portable());
        assert!(!status.is_broken());
    }

    #[test]
    fn reports_dangling_when_the_administration_directory_is_gone() {
        let fixture = TempDir::new().unwrap();
        let root = fixture.path();
        let (store, worktree) = linked_pair(root, "repo");
        fs::remove_dir_all(store.join("worktrees")).unwrap();

        let status = WorktreeLinkage::probe(&worktree, root);

        assert!(
            matches!(status, LinkageStatus::Dangling { .. }),
            "{status:?}"
        );
        assert!(status.is_broken());
    }

    /// The production defect: the pointer resolves on the node that wrote it,
    /// but names storage no other node mounts.
    #[test]
    fn reports_outside_shared_root_when_the_repository_is_node_local() {
        let fixture = TempDir::new().unwrap();
        let shared_root = fixture.path().join("shared");
        let elsewhere = fixture.path().join("srv-src");
        let (_store, worktree) = linked_pair(&elsewhere, "repo");
        fs::create_dir_all(&shared_root).unwrap();

        let status = WorktreeLinkage::probe(&worktree, &shared_root);

        assert!(
            matches!(status, LinkageStatus::OutsideSharedRoot { .. }),
            "{status:?}"
        );
        assert!(status.is_broken());
    }

    #[test]
    fn reports_back_pointer_mismatch_when_only_one_direction_was_repaired() {
        let fixture = TempDir::new().unwrap();
        let root = fixture.path();
        let (store, worktree) = linked_pair(root, "repo");
        fs::write(
            store.join("worktrees").join("repo").join("gitdir"),
            format!("{}\n", root.join("ws").join("other").join(".git").display()),
        )
        .unwrap();

        let status = WorktreeLinkage::probe(&worktree, root);

        assert!(
            matches!(status, LinkageStatus::BackPointerMismatch { .. }),
            "{status:?}"
        );
        assert!(status.is_broken());
    }

    #[test]
    fn reports_own_repository_for_a_real_git_directory() {
        let fixture = TempDir::new().unwrap();
        let worktree = fixture.path().join("repo");
        fs::create_dir_all(worktree.join(".git")).unwrap();

        assert_eq!(
            WorktreeLinkage::probe(&worktree, fixture.path()),
            LinkageStatus::OwnRepository
        );
    }

    #[test]
    fn reports_missing_when_there_is_no_git_entry() {
        let fixture = TempDir::new().unwrap();
        let worktree = fixture.path().join("not-a-repo");
        fs::create_dir_all(&worktree).unwrap();

        assert_eq!(
            WorktreeLinkage::probe(&worktree, fixture.path()),
            LinkageStatus::Missing
        );
    }

    /// An unparseable `.git` is unknown, never healthy. Guessing here would
    /// point a live workspace at the wrong repository.
    #[test]
    fn reports_indeterminate_for_an_unparseable_git_file() {
        let fixture = TempDir::new().unwrap();
        let worktree = fixture.path().join("repo");
        fs::create_dir_all(&worktree).unwrap();
        fs::write(worktree.join(".git"), "not a pointer\n").unwrap();

        let status = WorktreeLinkage::probe(&worktree, fixture.path());

        assert!(
            matches!(status, LinkageStatus::Indeterminate { .. }),
            "{status:?}"
        );
        assert!(!status.is_portable());
        assert!(!status.is_broken(), "unknown is not the same as broken");
    }

    /// A registration whose `commondir` is unreadable is unknown, not portable.
    #[test]
    fn reports_indeterminate_when_commondir_is_absent() {
        let fixture = TempDir::new().unwrap();
        let root = fixture.path();
        let (store, worktree) = linked_pair(root, "repo");
        fs::remove_file(store.join("worktrees").join("repo").join("commondir")).unwrap();

        let status = WorktreeLinkage::probe(&worktree, root);

        assert!(
            matches!(status, LinkageStatus::Indeterminate { .. }),
            "{status:?}"
        );
    }

    /// Canonicalisation happens before the containment test, so a symlink that
    /// escapes the shared root cannot pass as portable.
    #[cfg(unix)]
    #[test]
    fn rejects_a_symlink_that_escapes_the_shared_root() {
        let fixture = TempDir::new().unwrap();
        let shared_root = fixture.path().join("shared");
        let outside = fixture.path().join("outside");
        let (store, worktree) = linked_pair(&outside, "repo");
        fs::create_dir_all(&shared_root).unwrap();
        std::os::unix::fs::symlink(&store, shared_root.join("store")).unwrap();

        let status = WorktreeLinkage::probe(&worktree, &shared_root);

        assert!(
            matches!(status, LinkageStatus::OutsideSharedRoot { .. }),
            "{status:?}"
        );
    }

    /// `git -C` walks up to find a repository, so a directory nested under one
    /// can look healthy to a naive check. A `.git` *file* is required here, and
    /// its absence is `Missing` — never `Portable` inherited from an ancestor.
    #[test]
    fn does_not_inherit_portability_from_an_ancestor_repository() {
        let fixture = TempDir::new().unwrap();
        let root = fixture.path();
        let (_store, worktree) = linked_pair(root, "repo");
        let nested = worktree.join("packages").join("inner");
        fs::create_dir_all(&nested).unwrap();

        assert_eq!(
            WorktreeLinkage::probe(&nested, root),
            LinkageStatus::Missing
        );
    }

    #[test]
    fn resolves_a_relative_gitdir_pointer_against_the_worktree() {
        let fixture = TempDir::new().unwrap();
        let root = fixture.path();
        let (_store, worktree) = linked_pair(root, "repo");
        fs::write(
            worktree.join(".git"),
            "gitdir: ../../store/worktrees/repo\n",
        )
        .unwrap();

        assert!(
            WorktreeLinkage::probe(&worktree, root).is_portable(),
            "a relative pointer is still a pointer"
        );
    }
}
