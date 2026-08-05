use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, Mutex, OnceLock},
    time::Duration,
};

static WORKSPACE_DIR_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

use chrono::{TimeDelta, Utc};
use db::models::repository_admin_lock::RepositoryAdminLock;
use git::{GitCli, GitService, GitServiceError};
use sqlx::SqlitePool;
use thiserror::Error;
use tracing::{debug, info, trace};
use utils::{path::normalize_macos_private_alias, shell::resolve_executable_path};
use uuid::Uuid;

// Every mutation of a repository's shared `.git/worktrees` namespace uses the
// same repository-scoped lock. Per-worktree locks still permit repo-wide prune
// to race with a create/remove in another workspace.
static REPOSITORY_OPERATION_LOCKS: LazyLock<Mutex<HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static REPOSITORY_ADMIN_LOCKS: LazyLock<Mutex<HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Coordinator-side repository lock combining process-local exclusion with a
/// monotonically fenced SQLite lease.
#[derive(Clone)]
pub struct RepositoryAdminLockManager {
    pool: SqlitePool,
    lease_duration: Duration,
}

#[cfg(test)]
mod repository_admin_lock_tests {
    use std::time::Duration;

    use sqlx::sqlite::SqlitePoolOptions;
    use uuid::Uuid;

    use super::{RepositoryAdminLockManager, WorktreeError, canonical_lock_key};

    async fn manager() -> RepositoryAdminLockManager {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            r#"
            CREATE TABLE repository_admin_locks (
                repo_id BLOB PRIMARY KEY NOT NULL,
                generation INTEGER NOT NULL,
                operation_id BLOB NOT NULL,
                acquired_at TEXT NOT NULL,
                lease_expires_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        RepositoryAdminLockManager::new(pool, Duration::from_secs(30)).unwrap()
    }

    #[test]
    fn lock_keys_must_be_absolute() {
        assert!(matches!(
            canonical_lock_key(std::path::Path::new("relative")),
            Err(WorktreeError::InvalidPath(_))
        ));
    }

    #[tokio::test]
    async fn local_repository_operations_are_serialized_and_fenced() {
        let manager = manager().await;
        let repo_id = Uuid::new_v4();
        let path = tempfile::tempdir().unwrap();
        let first = manager.acquire(repo_id, path.path()).await.unwrap();
        assert_eq!(first.generation(), 1);

        let waiting_manager = manager.clone();
        let waiting_path = path.path().to_path_buf();
        let waiting =
            tokio::spawn(async move { waiting_manager.acquire(repo_id, &waiting_path).await });
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!waiting.is_finished());

        first.release().await.unwrap();
        let second = waiting.await.unwrap().unwrap();
        assert_eq!(second.generation(), 2);
        assert_ne!(second.operation_id(), Uuid::nil());
        second.release().await.unwrap();
    }
}

pub struct RepositoryAdminGuard {
    pool: SqlitePool,
    record: RepositoryAdminLock,
    _local_guard: tokio::sync::OwnedMutexGuard<()>,
}

impl RepositoryAdminLockManager {
    pub fn new(pool: SqlitePool, lease_duration: Duration) -> Result<Self, WorktreeError> {
        if lease_duration.is_zero() {
            return Err(WorktreeError::InvalidLeaseDuration);
        }
        Ok(Self {
            pool,
            lease_duration,
        })
    }

    pub async fn acquire(
        &self,
        repository_id: Uuid,
        repository_path: &Path,
    ) -> Result<RepositoryAdminGuard, WorktreeError> {
        let key = canonical_lock_key(repository_path)?;
        let mutex = {
            let mut locks = REPOSITORY_ADMIN_LOCKS.lock().unwrap();
            locks
                .entry(key)
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
                .clone()
        };
        let local_guard = mutex.lock_owned().await;
        let now = Utc::now();
        let lease_expires_at = now
            + TimeDelta::from_std(self.lease_duration)
                .map_err(|_| WorktreeError::InvalidLeaseDuration)?;
        let operation_id = Uuid::new_v4();
        let record = RepositoryAdminLock::acquire(
            &self.pool,
            repository_id,
            operation_id,
            now,
            lease_expires_at,
        )
        .await?
        .ok_or(WorktreeError::RepositoryLockBusy(repository_id))?;

        Ok(RepositoryAdminGuard {
            pool: self.pool.clone(),
            record,
            _local_guard: local_guard,
        })
    }
}

impl RepositoryAdminGuard {
    pub fn generation(&self) -> i64 {
        self.record.generation
    }

    pub fn operation_id(&self) -> Uuid {
        self.record.operation_id
    }

    pub async fn release(self) -> Result<(), WorktreeError> {
        let released = RepositoryAdminLock::release(
            &self.pool,
            self.record.repo_id,
            self.record.generation,
            self.record.operation_id,
        )
        .await?;
        if !released {
            return Err(WorktreeError::RepositoryLockLost(self.record.repo_id));
        }
        Ok(())
    }
}

fn canonical_lock_key(path: &Path) -> Result<PathBuf, WorktreeError> {
    if !path.is_absolute() {
        return Err(WorktreeError::InvalidPath(path.display().to_string()));
    }
    // A caller may identify the repository through one of its worktrees. Git's
    // common directory is the actual shared administration namespace and must
    // therefore map every such alias to the same lock.
    let authority_path = GitService::new()
        .get_common_dir(path)
        .unwrap_or_else(|_| path.to_path_buf());
    Ok(dunce::canonicalize(&authority_path).unwrap_or(authority_path))
}

fn repository_operation_lock(path: &Path) -> Result<Arc<tokio::sync::Mutex<()>>, WorktreeError> {
    let key = canonical_lock_key(path)?;
    let mut locks = REPOSITORY_OPERATION_LOCKS.lock().unwrap();
    Ok(locks
        .entry(key)
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone())
}

async fn finish_fenced_operation(
    operation: Result<(), WorktreeError>,
    guard: RepositoryAdminGuard,
) -> Result<(), WorktreeError> {
    let release = guard.release().await;
    match operation {
        Err(error) => Err(error),
        Ok(()) => release,
    }
}

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
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("repository administration lock for {0} is held by another owner")]
    RepositoryLockBusy(Uuid),
    #[error("repository administration lock for {0} was replaced or expired")]
    RepositoryLockLost(Uuid),
    #[error("repository administration lease duration must be positive and representable")]
    InvalidLeaseDuration,
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
        let lock = repository_operation_lock(repo_path)?;
        let _guard = lock.lock().await;
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

        Self::ensure_worktree_exists_unlocked(repo_path, branch_name, worktree_path).await
    }

    pub async fn create_worktree_fenced(
        lock_manager: &RepositoryAdminLockManager,
        repository_id: Uuid,
        repo_path: &Path,
        branch_name: &str,
        worktree_path: &Path,
        base_branch: &str,
        create_branch: bool,
    ) -> Result<(), WorktreeError> {
        let guard = lock_manager.acquire(repository_id, repo_path).await?;
        let result = Self::create_worktree(
            repo_path,
            branch_name,
            worktree_path,
            base_branch,
            create_branch,
        )
        .await;
        finish_fenced_operation(result, guard).await
    }

    /// Ensure worktree exists, recreating if necessary with proper synchronization
    /// This is the main entry point for ensuring a worktree exists and prevents race conditions
    pub async fn ensure_worktree_exists(
        repo_path: &Path,
        branch_name: &str,
        worktree_path: &Path,
    ) -> Result<(), WorktreeError> {
        let lock = repository_operation_lock(repo_path)?;
        let _guard = lock.lock().await;
        Self::ensure_worktree_exists_unlocked(repo_path, branch_name, worktree_path).await
    }

    pub async fn ensure_worktree_exists_fenced(
        lock_manager: &RepositoryAdminLockManager,
        repository_id: Uuid,
        repo_path: &Path,
        branch_name: &str,
        worktree_path: &Path,
    ) -> Result<(), WorktreeError> {
        let guard = lock_manager.acquire(repository_id, repo_path).await?;
        let result = Self::ensure_worktree_exists(repo_path, branch_name, worktree_path).await;
        finish_fenced_operation(result, guard).await
    }

    async fn ensure_worktree_exists_unlocked(
        repo_path: &Path,
        branch_name: &str,
        worktree_path: &Path,
    ) -> Result<(), WorktreeError> {
        let path_str = worktree_path.to_string_lossy().to_string();

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

        // Repair couldn't reconnect the worktree, so it must be recreated. Before
        // the destructive cleanup, move any directory that still holds data worth
        // keeping (uncommitted/untracked changes or an installed `node_modules`)
        // aside to a sibling `<name>.recovered-<epoch>` rather than deleting it,
        // so a forced recreation never silently destroys user work.
        if let Some(recovered) =
            Self::preserve_worktree_dir_if_valuable(&worktree_path_owned).await?
        {
            info!(
                "Preserved existing worktree data at {} before recreating {}",
                recovered.display(),
                path_str
            );
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

    /// If the worktree directory holds data worth keeping — git-visible
    /// uncommitted/untracked changes, or an installed `node_modules` whose
    /// reinstall is costly — move it aside to a sibling `<name>.recovered-<epoch>`
    /// directory instead of letting the forced recreation delete it. Returns the
    /// recovery path when a move happened, `None` when there was nothing worth
    /// preserving (so the caller proceeds with normal cleanup).
    async fn preserve_worktree_dir_if_valuable(
        worktree_path: &Path,
    ) -> Result<Option<PathBuf>, WorktreeError> {
        let worktree_path_owned = worktree_path.to_path_buf();
        tokio::task::spawn_blocking(move || -> Result<Option<PathBuf>, WorktreeError> {
            if !worktree_path_owned.exists()
                || !Self::worktree_has_recoverable_data(&worktree_path_owned)
            {
                return Ok(None);
            }

            let epoch = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let file_name = worktree_path_owned
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "worktree".to_string());

            // Never clobber an existing recovery dir (e.g. two recreations in the
            // same second, or a prior preserved copy).
            let mut recovered =
                worktree_path_owned.with_file_name(format!("{file_name}.recovered-{epoch}"));
            let mut suffix = 1;
            while recovered.exists() {
                recovered = worktree_path_owned
                    .with_file_name(format!("{file_name}.recovered-{epoch}-{suffix}"));
                suffix += 1;
            }

            fs::rename(&worktree_path_owned, &recovered).map_err(WorktreeError::Io)?;
            tracing::warn!(
                "Preserved worktree data before recreation: moved {} -> {}",
                worktree_path_owned.display(),
                recovered.display()
            );
            Ok(Some(recovered))
        })
        .await
        .map_err(|e| WorktreeError::TaskJoin(format!("{e}")))?
    }

    /// Whether a worktree directory contains data a delete would irrecoverably or
    /// expensively destroy: git-visible uncommitted/untracked changes, or a
    /// populated `node_modules`.
    fn worktree_has_recoverable_data(worktree_path: &Path) -> bool {
        if GitCli::new().has_changes(worktree_path).unwrap_or(false) {
            return true;
        }
        worktree_path.join("node_modules").exists()
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
        // Every unreadable entry is an error, not a miss. Under clustering this
        // directory holds one registration per live workspace of the repository
        // and lives on NFS, where a transient read failure is ordinary; silently
        // dropping entries would return `Ok(None)` and send the caller on to a
        // broader cleanup against every other workspace's metadata.
        let mut worktree_metadata_folders = Vec::new();
        match fs::read_dir(&worktree_metadata_path) {
            Ok(read_dir) => {
                for entry in read_dir {
                    worktree_metadata_folders.push(entry.map_err(|e| {
                        WorktreeError::Repository(format!(
                            "Failed to read an entry of worktree metadata directory at {}: {}",
                            worktree_metadata_path.display(),
                            e
                        ))
                    })?);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(WorktreeError::Repository(format!(
                    "Failed to read worktree metadata directory at {}: {}",
                    worktree_metadata_path.display(),
                    e
                )));
            }
        }
        // read the worktrees/*/gitdir and see which one matches the worktree_path
        for entry in worktree_metadata_folders {
            let gitdir_path = entry.path().join("gitdir");
            // `fs::exists` rather than `Path::exists`: the latter reports
            // `false` for "stat failed" as well as "absent", which would turn an
            // indeterminate registration into an invisible one.
            match fs::exists(&gitdir_path) {
                Ok(false) => continue,
                Err(e) => {
                    return Err(WorktreeError::Repository(format!(
                        "Failed to stat worktree registration at {}: {}",
                        gitdir_path.display(),
                        e
                    )));
                }
                Ok(true) => {}
            }
            let gitdir_content = fs::read_to_string(&gitdir_path).map_err(|e| {
                WorktreeError::Repository(format!(
                    "Failed to read worktree registration at {}: {}",
                    gitdir_path.display(),
                    e
                ))
            })?;
            if normalize_macos_private_alias(Path::new(gitdir_content.trim()))
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
        info!("Performing destructive cleanup for worktree: {worktree_display_name}");

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
            // `info!`, not `debug!`: this deletes a working tree, and a
            // destructive step logged below the default level is invisible
            // exactly when someone is trying to explain where their work went.
            info!("Removing worktree directory: {}", worktree_path.display());
            std::fs::remove_dir_all(worktree_path).map_err(WorktreeError::Io)?;
        }

        // Deliberately no `git worktree prune` here. Steps 1 and 2 already
        // removed *this* worktree's registration by resolved path, so a prune
        // could only ever touch registrations belonging to somebody else.
        //
        // That was tolerable when a repository's `worktrees/` directory held a
        // handful of entries owned by one node. It is not tolerable now: a
        // clustered repository's registrations all live in one shared store, so
        // this call would reach every other workspace of the repository —
        // including workspaces owned by other nodes. Prune decides by asking
        // whether a worktree directory is present, and over NFS a momentarily
        // unreadable directory is indistinguishable from a deleted one, so
        // cleaning up one workspace could unregister live ones. The same
        // operation has already cost this fleet a production build once, when it
        // walked registrations it did not own and died partway through.
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

        // Try to determine the git repo path if not provided
        let resolved_repo_path = if let Some(repo_path) = &worktree.git_repo_path {
            Some(repo_path.to_path_buf())
        } else {
            Self::infer_git_repo_path(&worktree.worktree_path).await
        };

        let lock_path = resolved_repo_path
            .as_deref()
            .unwrap_or(&worktree.worktree_path);
        let lock = repository_operation_lock(lock_path)?;
        let _guard = lock.lock().await;

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

    pub async fn cleanup_worktree_fenced(
        lock_manager: &RepositoryAdminLockManager,
        repository_id: Uuid,
        worktree: &WorktreeCleanup,
    ) -> Result<(), WorktreeError> {
        let repo_path = worktree.git_repo_path.as_deref().ok_or_else(|| {
            WorktreeError::InvalidPath(
                "fenced cleanup requires an authoritative repository path".into(),
            )
        })?;
        let guard = lock_manager.acquire(repository_id, repo_path).await?;
        let result = Self::cleanup_worktree(worktree).await;
        finish_fenced_operation(result, guard).await
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
        let lock = repository_operation_lock(repo_path)?;
        let _guard = lock.lock().await;
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

#[tokio::test]
async fn concurrent_create_remove_and_prune_share_repository_lock() {
    use tempfile::TempDir;

    let td = TempDir::new().unwrap();
    let repo_path = td.path().join("repo");
    GitService::new()
        .initialize_repo_with_main_branch(&repo_path)
        .unwrap();
    let first_path = td.path().join("first");
    let second_path = td.path().join("second");

    let create_first =
        WorktreeManager::create_worktree(&repo_path, "concurrent-first", &first_path, "main", true);
    let create_second = WorktreeManager::create_worktree(
        &repo_path,
        "concurrent-second",
        &second_path,
        "main",
        true,
    );
    let (first_result, second_result) = tokio::join!(create_first, create_second);
    first_result.unwrap();
    second_result.unwrap();
    assert!(first_path.join(".git").is_file());
    assert!(second_path.join(".git").is_file());

    let first_cleanup = WorktreeCleanup::new(first_path.clone(), Some(repo_path.clone()));
    let remove_first = WorktreeManager::cleanup_worktree(&first_cleanup);
    let ensure_second =
        WorktreeManager::ensure_worktree_exists(&repo_path, "concurrent-second", &second_path);
    let (remove_result, ensure_result) = tokio::join!(remove_first, ensure_second);
    remove_result.unwrap();
    ensure_result.unwrap();

    assert!(!first_path.exists());
    assert!(second_path.join(".git").is_file());
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

/// When a forced recreation cannot preserve the worktree in place (repair fails
/// or the branch mismatches), a directory holding recoverable data (uncommitted
/// changes or `node_modules`) must be moved aside, not deleted.
#[tokio::test]
async fn forced_recreation_moves_recoverable_data_aside() {
    use std::fs;

    use tempfile::TempDir;
    let td = TempDir::new().unwrap();

    let repo_path = td.path().join("repo");
    let git_service = GitService::new();
    git_service
        .initialize_repo_with_main_branch(&repo_path)
        .unwrap();
    git_service
        .create_branch(&repo_path, "wanted", "main")
        .unwrap();

    let wt_path = td.path().join("wt");
    WorktreeManager::create_worktree(&repo_path, "other", &wt_path, "main", true)
        .await
        .unwrap();

    // An installed node_modules plus an untracked file — data a delete would
    // destroy.
    let node_modules = wt_path.join("node_modules");
    fs::create_dir_all(&node_modules).unwrap();
    fs::write(node_modules.join("marker.txt"), b"installed").unwrap();

    // Break the admin linkage so recreation is forced; the branch mismatch
    // ("other" vs "wanted") stops the in-place repair from keeping it.
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

    WorktreeManager::ensure_worktree_exists(&repo_path, "wanted", &wt_path)
        .await
        .unwrap();

    // Fresh worktree exists on the expected branch.
    assert_eq!(git_service.get_current_branch(&wt_path).unwrap(), "wanted");

    // The old data was moved aside to a sibling `.recovered-*` dir, not deleted.
    let recovered = fs::read_dir(td.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().starts_with("wt.recovered-"))
                .unwrap_or(false)
        })
        .expect("a wt.recovered-* directory should exist");
    assert!(
        recovered.join("node_modules").join("marker.txt").exists(),
        "recovered directory should contain the preserved node_modules"
    );
}

/// Cleaning up one worktree must not disturb another worktree of the same
/// repository.
///
/// This is cheap to get wrong and expensive to discover. Cleanup used to finish
/// with a repository-wide `git worktree prune`, which was tolerable while a
/// repository's registrations belonged to one node and one workspace at a time.
/// Under clustering every workspace of a repository shares one store, so that
/// prune reached other workspaces' registrations — and on network storage a
/// directory that is momentarily unreadable looks exactly like a deleted one.
#[tokio::test]
async fn cleanup_of_one_worktree_leaves_a_sibling_registration_intact() {
    use tempfile::TempDir;
    let td = TempDir::new().unwrap();

    let repo_path = td.path().join("repo");
    let git_service = GitService::new();
    git_service
        .initialize_repo_with_main_branch(&repo_path)
        .unwrap();

    let doomed = td.path().join("ws-doomed").join("repo");
    let survivor = td.path().join("ws-survivor").join("repo");
    WorktreeManager::create_worktree(&repo_path, "vk/doomed", &doomed, "main", true)
        .await
        .unwrap();
    WorktreeManager::create_worktree(&repo_path, "vk/survivor", &survivor, "main", true)
        .await
        .unwrap();

    // Both worktrees share one `worktrees/` namespace, and git names their
    // registrations after the path basename — which is identical here, so the
    // second is disambiguated with a suffix. Resolve each by path.
    let survivor_registration =
        WorktreeManager::find_worktree_git_internal_name(&repo_path, &survivor)
            .unwrap()
            .expect("the surviving worktree should be registered");

    WorktreeManager::cleanup_worktree(&WorktreeCleanup::new(
        doomed.clone(),
        Some(repo_path.clone()),
    ))
    .await
    .unwrap();

    assert!(!doomed.exists(), "the targeted worktree should be gone");
    assert!(
        survivor.join(".git").is_file(),
        "the sibling worktree directory must survive"
    );
    assert_eq!(
        WorktreeManager::find_worktree_git_internal_name(&repo_path, &survivor).unwrap(),
        Some(survivor_registration),
        "the sibling's registration must survive under the same name"
    );
    assert_eq!(
        git_service.get_current_branch(&survivor).unwrap(),
        "vk/survivor",
        "the sibling worktree must still be usable"
    );
}
