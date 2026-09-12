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
