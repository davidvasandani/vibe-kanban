use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, Mutex, OnceLock},
};

static WORKSPACE_DIR_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

use git::{GitService, GitServiceError};
use thiserror::Error;
use tracing::{debug, info, trace};
use utils::{path::normalize_macos_private_alias, shell::resolve_executable_path};

// Global synchronization for worktree creation to prevent race conditions
static WORKTREE_CREATION_LOCKS: LazyLock<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone)]
pub struct WorktreeCleanup {
    pub worktree_path: PathBuf,
    pub git_repo_path: Option<PathBuf>,
}

impl WorktreeCleanup {
    pub fn new(worktree_path: PathBuf, git_repo_path: Option<PathBuf>) -> Self {
        Self {
            worktree_path,
            git_repo_path,
        }
    }
}

#[derive(Debug, Error)]
pub enum WorktreeError {
    #[error(transparent)]
    GitService(#[from] GitServiceError),
    #[error("Git CLI error: {0}")]
    GitCli(String),
    #[error("Task join error: {0}")]
    TaskJoin(String),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Branch not found: {0}")]
    BranchNotFound(String),
    #[error("Repository error: {0}")]
    Repository(String),
}

pub struct WorktreeManager;

impl WorktreeManager {
    pub fn set_workspace_dir_override(path: PathBuf) {
        let _ = WORKSPACE_DIR_OVERRIDE.set(path);
    }

    /// Create a worktree with a new branch
    pub async fn create_worktree(
        repo_path: &Path,
        branch_name: &str,
        worktree_path: &Path,
        base_branch: &str,
        create_branch: bool,
    ) -> Result<(), WorktreeError> {
        if create_branch {
            let repo_path_owned = repo_path.to_path_buf();
            let branch_name_owned = branch_name.to_string();
            let base_branch_owned = base_branch.to_string();

            tokio::task::spawn_blocking(move || {
                GitService::new().create_branch(
                    &repo_path_owned,
                    &branch_name_owned,
                    &base_branch_owned,
                )
            })
            .await
            .map_err(|e| WorktreeError::TaskJoin(format!("Task join error: {e}")))??;
        }

        Self::ensure_worktree_exists(repo_path, branch_name, worktree_path).await
    }

    /// Ensure worktree exists, recreating if necessary with proper synchronization
    /// This is the main entry point for ensuring a worktree exists and prevents race conditions
    pub async fn ensure_worktree_exists(
        repo_path: &Path,
        branch_name: &str,
        worktree_path: &Path,
    ) -> Result<(), WorktreeError> {
        let path_str = worktree_path.to_string_lossy().to_string();

        // Get or create a lock for this specific worktree path
        let lock = {
            let mut locks = WORKTREE_CREATION_LOCKS.lock().unwrap();
            locks
                .entry(path_str.clone())
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
                .clone()
        };

        // Acquire the lock for this specific worktree path
        let _guard = lock.lock().await;

        // Check if worktree already exists and is properly set up
        if Self::is_worktree_properly_set_up(repo_path, worktree_path).await? {
            trace!("Worktree already properly set up at path: {}", path_str);
            return Ok(());
        }

        // If worktree doesn't exist or isn't properly set up, recreate it
        info!("Worktree needs recreation at path: {}", path_str);
        Self::recreate_worktree_internal(repo_path, branch_name, worktree_path).await
    }

    /// Internal worktree recreation function (always recreates)
    async fn recreate_worktree_internal(
        repo_path: &Path,
        branch_name: &str,
        worktree_path: &Path,
    ) -> Result<(), WorktreeError> {
        let path_str = worktree_path.to_string_lossy().to_string();
        let branch_name_owned = branch_name.to_string();
        let worktree_path_owned = worktree_path.to_path_buf();

        info!(
            "Creating worktree {} at path {}",
            branch_name_owned, path_str
        );

        // Step 0: Attempt a non-destructive in-place repair first. On restart the
        // working directory is usually intact and only git's administrative
        // linkage has drifted; repairing it preserves untracked files (e.g.
        // `node_modules`) and uncommitted changes that a delete+recreate would
        // wipe. Only fall through to destructive recreation if repair can't
        // reconnect the worktree on the expected branch.
        if Self::try_repair_worktree_in_place(repo_path, &branch_name_owned, &worktree_path_owned)
            .await?
        {
            info!(
                "Repaired existing worktree {} in place at {} (preserved working tree)",
                branch_name_owned, path_str
            );
            return Ok(());
        }

        // Step 1: Comprehensive cleanup of existing worktree and metadata (non-blocking)
        Self::comprehensive_worktree_cleanup_async(repo_path, &worktree_path_owned).await?;

        // Step 2: Ensure parent directory exists (non-blocking)
        if let Some(parent) = worktree_path_owned.parent() {
            let parent_path = parent.to_path_buf();
            tokio::task::spawn_blocking(move || std::fs::create_dir_all(&parent_path))
                .await
                .map_err(|e| WorktreeError::TaskJoin(format!("Task join error: {e}")))?
                .map_err(WorktreeError::Io)?;
        }

        // Step 3: Create the worktree with retry logic for metadata conflicts (non-blocking)
        Self::create_worktree_with_retry(
            repo_path,
            &branch_name_owned,
            &worktree_path_owned,
            &path_str,
        )
        .await
    }

    /// Check if a worktree is properly set up (filesystem + git metadata)
    async fn is_worktree_properly_set_up(
        repo_path: &Path,
        worktree_path: &Path,
    ) -> Result<bool, WorktreeError> {
        let repo_path = repo_path.to_path_buf();
        let worktree_path = worktree_path.to_path_buf();

        tokio::task::spawn_blocking(move || -> Result<bool, WorktreeError> {
            // Check 1: Filesystem path must exist
            if !worktree_path.exists() {
                return Ok(false);
            }

            // Check 2: Worktree must be registered in git metadata using find_worktree
            let git_service = GitService::new();
            let Some(worktree_name) =
                Self::find_worktree_git_internal_name(&repo_path, &worktree_path)?
            else {
                // Directory exists but not registered in git metadata - needs recreation
                return Ok(false);
            };

            // Try to find the worktree - if it exists and is valid, we're good
            Ok(git_service.validate_worktree(&repo_path, &worktree_name)?)
        })
        .await
        .map_err(|e| WorktreeError::TaskJoin(format!("{e}")))?
    }

    /// Try to reconnect an existing worktree directory to its repository without
    /// destroying its contents.
    ///
    /// Returns `Ok(true)` only when, after the repair attempt, the worktree is
    /// properly set up AND checked out on the expected branch — in which case the
    /// caller can skip destructive recreation and keep the working tree (untracked
    /// files like `node_modules` and uncommitted changes) intact. Returns
    /// `Ok(false)` when the directory is missing, isn't a git worktree, or can't
    /// be repaired onto the expected branch, so the caller must recreate it.
    async fn try_repair_worktree_in_place(
        repo_path: &Path,
        branch_name: &str,
        worktree_path: &Path,
    ) -> Result<bool, WorktreeError> {
        // Only meaningful if the directory exists and carries a worktree marker.
        let git_marker = worktree_path.join(".git");
        if !worktree_path.exists() || !git_marker.exists() {
            return Ok(false);
        }

        let repo_path_owned = repo_path.to_path_buf();
        let worktree_path_owned = worktree_path.to_path_buf();
        let branch_name_owned = branch_name.to_string();

        tokio::task::spawn_blocking(move || {
            let git_service = GitService::new();
            if let Err(e) = git_service.repair_worktree(&repo_path_owned, &worktree_path_owned) {
                debug!(
                    "git worktree repair failed for {} (will fall back to recreation): {}",
                    worktree_path_owned.display(),
                    e
                );
            }
        })
        .await
        .map_err(|e| WorktreeError::TaskJoin(format!("{e}")))?;

        // Repair only rewrites admin files; the branch checked out in the
        // directory is whatever was there before. Confirm it matches what the
        // workspace expects before trusting the repaired worktree, otherwise
        // recreate so we don't silently keep a stale/wrong branch.
        if !Self::is_worktree_properly_set_up(repo_path, worktree_path).await? {
            return Ok(false);
        }

        let worktree_path_owned = worktree_path.to_path_buf();
        let branch_matches = tokio::task::spawn_blocking(move || {
            match GitService::new().get_current_branch(&worktree_path_owned) {
                Ok(current) => current == branch_name_owned,
                Err(e) => {
                    debug!(
                        "Could not read current branch of repaired worktree {}: {}",
                        worktree_path_owned.display(),
                        e
                    );
                    false
                }
            }
        })
        .await
        .map_err(|e| WorktreeError::TaskJoin(format!("{e}")))?;

        Ok(branch_matches)
    }

    fn find_worktree_git_internal_name(
        git_repo_path: &Path,
        worktree_path: &Path,
    ) -> Result<Option<String>, WorktreeError> {
        fn canonicalize_for_compare(path: &Path) -> PathBuf {
            dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
        }

        let worktree_root = canonicalize_for_compare(&normalize_macos_private_alias(worktree_path));
        let worktree_metadata_path = Self::get_worktree_metadata_path(git_repo_path)?;
        let worktree_metadata_folders = match fs::read_dir(&worktree_metadata_path) {
            Ok(read_dir) => read_dir
                .filter_map(|entry| entry.ok())
                .collect::<Vec<fs::DirEntry>>(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(WorktreeError::Repository(format!(
                    "Failed to read worktree metadata directory at {}: {}",
                    worktree_metadata_path.display(),
                    e
                )));
            }
        };
        // read the worktrees/*/gitdir and see which one matches the worktree_path
        for entry in worktree_metadata_folders {
            let gitdir_path = entry.path().join("gitdir");
            if gitdir_path.exists()
                && let Ok(gitdir_content) = fs::read_to_string(&gitdir_path)
                && normalize_macos_private_alias(Path::new(gitdir_content.trim()))
                    .parent()
                    .map(canonicalize_for_compare)
                    .is_some_and(|p| p == worktree_root)
            {
                return Ok(Some(entry.file_name().to_string_lossy().to_string()));
            }
        }
        Ok(None)
    }

    fn get_worktree_metadata_path(git_repo_path: &Path) -> Result<PathBuf, WorktreeError> {
        let git_service = GitService::new();
        Ok(git_service.get_common_dir(git_repo_path)?.join("worktrees"))
    }

    /// Comprehensive cleanup of worktree path and metadata to prevent "path exists" errors (blocking)
    fn comprehensive_worktree_cleanup(
        git_repo_path: &Path,
        worktree_path: &Path,
    ) -> Result<(), WorktreeError> {
        let worktree_display_name = worktree_path.to_string_lossy().to_string();
        debug!("Performing cleanup for worktree: {worktree_display_name}");

        // Step 1: Use GitService to remove the worktree registration (force) if present
        // The Git CLI is more robust than libgit2 for mutable worktree operations
        let git_service = GitService::new();
        if let Err(e) = git_service.remove_worktree(git_repo_path, worktree_path, true) {
            debug!("git worktree remove non-fatal error: {}", e);
        }

        // Step 2: Always force cleanup metadata directory (proactive cleanup)
        if let Err(e) = Self::force_cleanup_worktree_metadata(git_repo_path, worktree_path) {
            debug!("Metadata cleanup failed (non-fatal): {}", e);
        }

        // Step 3: Clean up physical worktree directory if it exists
        if worktree_path.exists() {
            debug!(
                "Removing existing worktree directory: {}",
                worktree_path.display()
            );
            std::fs::remove_dir_all(worktree_path).map_err(WorktreeError::Io)?;
        }

        // Step 4: Good-practice to clean up any other stale admin entries
        if let Err(e) = git_service.prune_worktrees(git_repo_path) {
            debug!("git worktree prune non-fatal error: {}", e);
        }

        debug!("Comprehensive cleanup completed for worktree: {worktree_display_name}",);
        Ok(())
    }

    /// Async version of comprehensive cleanup to avoid blocking the main runtime
    async fn comprehensive_worktree_cleanup_async(
        git_repo_path: &Path,
        worktree_path: &Path,
    ) -> Result<(), WorktreeError> {
        let git_repo_path_owned = git_repo_path.to_path_buf();
        let worktree_path_owned = worktree_path.to_path_buf();

        // Check if the repository can be opened
        let is_openable = tokio::task::spawn_blocking({
            let git_repo_path = git_repo_path_owned.clone();
            move || GitService::new().is_repo_openable(&git_repo_path)
        })
        .await
        .map_err(|e| WorktreeError::TaskJoin(format!("{e}")))?;

        if is_openable {
            // Repository exists, perform comprehensive cleanup
            tokio::task::spawn_blocking(move || {
                Self::comprehensive_worktree_cleanup(&git_repo_path_owned, &worktree_path_owned)
            })
            .await
            .map_err(|e| WorktreeError::TaskJoin(format!("Task join error: {e}")))?
        } else {
            // Repository doesn't exist (likely deleted project), fall back to simple cleanup
            debug!(
                "Failed to open repository at {:?}. Falling back to simple cleanup for worktree at {}",
                git_repo_path_owned,
                worktree_path_owned.display()
            );
            Self::simple_worktree_cleanup(&worktree_path_owned).await?;
            Ok(())
        }
    }

    /// Create worktree with retry logic in non-blocking manner
    async fn create_worktree_with_retry(
        git_repo_path: &Path,
        branch_name: &str,
        worktree_path: &Path,
        path_str: &str,
    ) -> Result<(), WorktreeError> {
        let git_repo_path = git_repo_path.to_path_buf();
        let branch_name = branch_name.to_string();
        let worktree_path = worktree_path.to_path_buf();
        let path_str = path_str.to_string();

        tokio::task::spawn_blocking(move || -> Result<(), WorktreeError> {
            // Prefer git CLI for worktree add to inherit sparse-checkout semantics
            let git_service = GitService::new();
            match git_service.add_worktree(&git_repo_path, &worktree_path, &branch_name, false) {
                Ok(()) => {
                    if !worktree_path.exists() {
                        return Err(WorktreeError::Repository(format!(
                            "Worktree creation reported success but path {path_str} does not exist"
                        )));
                    }
                    info!(
                        "Successfully created worktree {} at {} (git CLI)",
                        branch_name, path_str
                    );
                    Ok(())
                }
                Err(e) => {
                    tracing::warn!(
                        "git worktree add failed; attempting metadata cleanup and retry: {}",
                        e
                    );
                    // Force cleanup metadata and try one more time
                    Self::force_cleanup_worktree_metadata(&git_repo_path, &worktree_path)?;
                    // Clean up physical directory if it exists
                    // Needed if previous attempt failed after directory creation
                    if worktree_path.exists() {
                        std::fs::remove_dir_all(&worktree_path).map_err(WorktreeError::Io)?;
                    }
                    if let Err(e2) = git_service.add_worktree(
                        &git_repo_path,
                        &worktree_path,
                        &branch_name,
                        false,
                    ) {
                        return Err(WorktreeError::GitService(e2));
                    }
                    if !worktree_path.exists() {
                        return Err(WorktreeError::Repository(format!(
                            "Worktree creation reported success but path {path_str} does not exist"
                        )));
                    }
                    info!(
                        "Successfully created worktree {} at {} after metadata cleanup (git CLI)",
                        branch_name, path_str
                    );
                    Ok(())
                }
            }
        })
        .await
        .map_err(|e| WorktreeError::TaskJoin(format!("{e}")))?
    }

    /// Force cleanup worktree metadata directory
    fn force_cleanup_worktree_metadata(
        git_repo_path: &Path,
        worktree_path: &Path,
    ) -> Result<(), WorktreeError> {
        if let Some(worktree_name) =
            Self::find_worktree_git_internal_name(git_repo_path, worktree_path)?
        {
            let git_worktree_metadata_path =
                Self::get_worktree_metadata_path(git_repo_path)?.join(worktree_name);

            if git_worktree_metadata_path.exists() {
                debug!(
                    "Force removing git worktree metadata: {}",
                    git_worktree_metadata_path.display()
                );
                std::fs::remove_dir_all(&git_worktree_metadata_path)?;
            }
        }

        Ok(())
    }

    /// Clean up multiple worktrees
    pub async fn batch_cleanup_worktrees(data: &[WorktreeCleanup]) -> Result<(), WorktreeError> {
        for cleanup_data in data {
            tracing::debug!("Cleaning up worktree: {:?}", cleanup_data.worktree_path);

            if let Err(e) = Self::cleanup_worktree(cleanup_data).await {
                tracing::error!("Failed to cleanup worktree: {}", e);
            }
        }
        Ok(())
    }

    /// Clean up a worktree path and its git metadata (non-blocking)
    /// If git_repo_path is None, attempts to infer it from the worktree itself
    pub async fn cleanup_worktree(worktree: &WorktreeCleanup) -> Result<(), WorktreeError> {
        let path_str = worktree.worktree_path.to_string_lossy().to_string();

        // Get the same lock to ensure we don't interfere with creation
        let lock = {
            let mut locks = WORKTREE_CREATION_LOCKS.lock().unwrap();
            locks
                .entry(path_str.clone())
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
                .clone()
        };

        let _guard = lock.lock().await;

        // Try to determine the git repo path if not provided
        let resolved_repo_path = if let Some(repo_path) = &worktree.git_repo_path {
            Some(repo_path.to_path_buf())
        } else {
            Self::infer_git_repo_path(&worktree.worktree_path).await
        };

        if let Some(repo_path) = resolved_repo_path {
            Self::comprehensive_worktree_cleanup_async(&repo_path, &worktree.worktree_path).await?;
        } else {
            // Can't determine repo path, just clean up the worktree directory
            debug!(
                "Cannot determine git repo path for worktree {}, performing simple cleanup",
                path_str
            );
            Self::simple_worktree_cleanup(&worktree.worktree_path).await?;
        }

        Ok(())
    }

    /// Try to infer the git repository path from a worktree
    async fn infer_git_repo_path(worktree_path: &Path) -> Option<PathBuf> {
        // Try using git rev-parse --git-common-dir from within the worktree
        let worktree_path_owned = worktree_path.to_path_buf();

        let git_path = resolve_executable_path("git").await?;

        use utils::command_ext::NoWindowExt;
        let output = tokio::process::Command::new(git_path)
            .args(["rev-parse", "--git-common-dir"])
            .current_dir(&worktree_path_owned)
            .no_window()
            .output()
            .await
            .ok()?;

        if output.status.success() {
            let git_common_dir = String::from_utf8(output.stdout).ok()?.trim().to_string();

            // git-common-dir gives us the path to the .git directory
            // We need the working directory (parent of .git)
            let git_dir_path = Path::new(&git_common_dir);
            if git_dir_path.file_name() == Some(std::ffi::OsStr::new(".git")) {
                git_dir_path.parent()?.to_str().map(PathBuf::from)
            } else {
                // In case of bare repo or unusual setup, use the git-common-dir as is
                Some(PathBuf::from(git_common_dir))
            }
        } else {
            None
        }
    }

    /// Simple worktree cleanup when we can't determine the main repo
    async fn simple_worktree_cleanup(worktree_path: &Path) -> Result<(), WorktreeError> {
        let worktree_path_owned = worktree_path.to_path_buf();

        tokio::task::spawn_blocking(move || -> Result<(), WorktreeError> {
            if worktree_path_owned.exists() {
                std::fs::remove_dir_all(&worktree_path_owned).map_err(WorktreeError::Io)?;
                info!(
                    "Removed worktree directory: {}",
                    worktree_path_owned.display()
                );
            }
            Ok(())
        })
        .await
        .map_err(|e| WorktreeError::TaskJoin(format!("{e}")))?
    }

    /// Move a worktree to a new location
    pub async fn move_worktree(
        repo_path: &Path,
        old_path: &Path,
        new_path: &Path,
    ) -> Result<(), WorktreeError> {
        let repo_path = repo_path.to_path_buf();
        let old_path = old_path.to_path_buf();
        let new_path = new_path.to_path_buf();

        tokio::task::spawn_blocking(move || {
            let git_service = GitService::new();
            git_service
                .move_worktree(&repo_path, &old_path, &new_path)
                .map_err(WorktreeError::GitService)
        })
        .await
        .map_err(|e| WorktreeError::TaskJoin(format!("{e}")))?
    }

    /// Get the base directory for vibe-kanban worktrees
    pub fn get_worktree_base_dir() -> std::path::PathBuf {
        if let Some(override_path) = WORKSPACE_DIR_OVERRIDE.get() {
            // Always use app-owned subdirectory within custom path for safety.
            // This ensures orphan cleanup never touches user's existing folders.
            return override_path.join(".vibe-kanban-workspaces");
        }
        Self::get_default_worktree_base_dir()
    }

    /// Get the default base directory (ignoring any override)
    pub fn get_default_worktree_base_dir() -> std::path::PathBuf {
        utils::path::get_vibe_kanban_temp_dir().join("worktrees")
    }

    pub async fn cleanup_suspected_worktree(path: &Path) -> Result<bool, WorktreeError> {
        let git_marker = path.join(".git");
        if !git_marker.exists() || !git_marker.is_file() {
            return Ok(false);
        }

        debug!("Cleaning up suspected worktree at {}", path.display());
        let cleanup = WorktreeCleanup::new(path.to_path_buf(), None);
        Self::cleanup_worktree(&cleanup).await?;
        Ok(true)
    }
}

#[tokio::test]
async fn create_worktree_when_repo_path_is_a_worktree() {
    use tempfile::TempDir;
    let td = TempDir::new().unwrap();

    let repo_path = td.path().join("repo");
    let git_service = GitService::new();
    git_service
        .initialize_repo_with_main_branch(&repo_path)
        .unwrap();

    let base_worktree_path = td.path().join("wt-base");
    WorktreeManager::create_worktree(
        &repo_path,
        "wt-base-branch",
        &base_worktree_path,
        "main",
        true,
    )
    .await
    .unwrap();
    assert!(base_worktree_path.join(".git").is_file());

    let child_worktree_path = td.path().join("wt-child");
    WorktreeManager::create_worktree(
        &base_worktree_path,
        "wt-child-branch",
        &child_worktree_path,
        "main",
        true,
    )
    .await
    .unwrap();
    assert!(child_worktree_path.join(".git").is_file());

    // Regression: repo_path itself is a worktree (so `.git` is a file), but metadata lookup still works.
    WorktreeManager::ensure_worktree_exists(
        &base_worktree_path,
        "wt-child-branch",
        &child_worktree_path,
    )
    .await
    .unwrap();
}

/// Regression: when a worktree's git admin linkage has drifted (e.g. after a
/// vibe-kanban restart), `ensure_worktree_exists` must repair it in place rather
/// than deleting and recreating the directory, so untracked working-tree state
/// like `node_modules` survives.
#[tokio::test]
async fn ensure_worktree_repairs_in_place_and_preserves_untracked_files() {
    use std::fs;

    use tempfile::TempDir;
    let td = TempDir::new().unwrap();

    let repo_path = td.path().join("repo");
    let git_service = GitService::new();
    git_service
        .initialize_repo_with_main_branch(&repo_path)
        .unwrap();

    // Create a worktree on a feature branch.
    let wt_path = td.path().join("wt");
    WorktreeManager::create_worktree(&repo_path, "feat", &wt_path, "main", true)
        .await
        .unwrap();
    assert!(wt_path.join(".git").is_file());

    // Simulate expensive, untracked working-tree state (e.g. `node_modules`)
    // that a delete+recreate would wipe.
    let node_modules = wt_path.join("node_modules");
    fs::create_dir_all(&node_modules).unwrap();
    fs::write(node_modules.join("marker.txt"), b"installed").unwrap();

    // Break the repo-side admin `gitdir` pointer so the worktree is no longer
    // considered "properly set up" — this is what would otherwise force a
    // destructive recreation.
    let worktrees_meta = git_service
        .get_common_dir(&repo_path)
        .unwrap()
        .join("worktrees");
    let admin_dir = fs::read_dir(&worktrees_meta)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.is_dir())
        .expect("worktree admin dir should exist");
    fs::write(admin_dir.join("gitdir"), b"/nonexistent/broken/.git\n").unwrap();

    // Sanity: the broken pointer makes the worktree look unregistered.
    assert!(
        !WorktreeManager::is_worktree_properly_set_up(&repo_path, &wt_path)
            .await
            .unwrap(),
        "broken admin linkage should read as not-properly-set-up"
    );

    // The resume path should now repair in place instead of wiping the dir.
    WorktreeManager::ensure_worktree_exists(&repo_path, "feat", &wt_path)
        .await
        .unwrap();

    // The untracked node_modules must survive.
    assert!(
        node_modules.join("marker.txt").exists(),
        "node_modules should be preserved by in-place repair"
    );
    // And the worktree must be healthy again, on the expected branch.
    assert!(
        WorktreeManager::is_worktree_properly_set_up(&repo_path, &wt_path)
            .await
            .unwrap(),
        "worktree should be properly set up after repair"
    );
    assert_eq!(
        git_service.get_current_branch(&wt_path).unwrap(),
        "feat",
        "repaired worktree should remain on the expected branch"
    );
}

/// When recreation is forced (drifted linkage) but the on-disk directory is on
/// a different branch than the workspace expects, the repair path must NOT
/// accept it — it should fall back to destructive recreation onto the correct
/// branch rather than silently keeping the wrong branch.
#[tokio::test]
async fn ensure_worktree_recreates_when_branch_mismatches() {
    use std::fs;

    use tempfile::TempDir;
    let td = TempDir::new().unwrap();

    let repo_path = td.path().join("repo");
    let git_service = GitService::new();
    git_service
        .initialize_repo_with_main_branch(&repo_path)
        .unwrap();

    // The workspace's target branch already exists (as in the real resume flow,
    // where `ensure_worktree_exists` is only called for an existing branch).
    git_service
        .create_branch(&repo_path, "wanted", "main")
        .unwrap();

    // Create a worktree on the "wrong" branch at the path the workspace uses.
    let wt_path = td.path().join("wt");
    WorktreeManager::create_worktree(&repo_path, "other", &wt_path, "main", true)
        .await
        .unwrap();
    assert_eq!(git_service.get_current_branch(&wt_path).unwrap(), "other");

    // Break the admin linkage so recreation is forced (repair alone would keep
    // the "other" branch checked out).
    let admin_dir = git_service
        .get_common_dir(&repo_path)
        .unwrap()
        .join("worktrees");
    let admin_dir = fs::read_dir(&admin_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.is_dir())
        .expect("worktree admin dir should exist");
    fs::write(admin_dir.join("gitdir"), b"/nonexistent/broken/.git\n").unwrap();

    // Ensuring the "wanted" branch at this path must end with the worktree on
    // "wanted", not silently keep "other".
    WorktreeManager::ensure_worktree_exists(&repo_path, "wanted", &wt_path)
        .await
        .unwrap();
    assert_eq!(
        git_service.get_current_branch(&wt_path).unwrap(),
        "wanted",
        "worktree should be recreated onto the expected branch"
    );
}
