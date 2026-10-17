use std::path::{Path, PathBuf};

use db::{
    DBService,
    models::{
        file::WorkspaceAttachment,
        repo::{Repo, RepoError},
        requests::WorkspaceRepoInput,
        session::Session,
        workspace::Workspace as DbWorkspace,
        workspace_repo::{CreateWorkspaceRepo, RepoWithTargetBranch, WorkspaceRepo},
    },
};
use git::{GitService, GitServiceError};
use thiserror::Error;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use worktree_manager::{
    RepositoryAdminLockManager, WorktreeCleanup, WorktreeError, WorktreeManager,
};

const SHARED_REPOSITORIES_DIR: &str = "repositories";
const SHARED_WORKSPACES_DIR: &str = "workspaces";
const SHARED_EXECUTION_LOGS_DIR: &str = "execution-logs";

/// Canonical, host-independent paths within the cluster shared volume.
///
/// IDs, rather than user-controlled names, form every authoritative path so
/// all nodes derive exactly the same location without path traversal or naming
/// collisions. Display names may still be used inside workspace metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedWorkspacePaths {
    root: PathBuf,
}

impl SharedWorkspacePaths {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, WorkspaceError> {
        let root = root.into();
        if !root.is_absolute() {
            return Err(WorkspaceError::InvalidSharedRoot(root));
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn repositories_dir(&self) -> PathBuf {
        self.root.join(SHARED_REPOSITORIES_DIR)
    }

    pub fn repository_dir(&self, repository_id: Uuid) -> PathBuf {
        self.repositories_dir().join(repository_id.to_string())
    }

    pub fn workspaces_dir(&self) -> PathBuf {
        self.root.join(SHARED_WORKSPACES_DIR)
    }

    pub fn workspace_dir(&self, workspace_id: Uuid) -> PathBuf {
        self.workspaces_dir().join(workspace_id.to_string())
    }

    pub fn execution_logs_dir(&self) -> PathBuf {
        self.root.join(SHARED_EXECUTION_LOGS_DIR)
    }

    pub fn execution_log_dir(&self, execution_id: Uuid) -> PathBuf {
        self.execution_logs_dir().join(execution_id.to_string())
    }

    pub async fn create_base_dirs(&self) -> Result<(), std::io::Error> {
        for path in [
            self.repositories_dir(),
            self.workspaces_dir(),
            self.execution_logs_dir(),
        ] {
            tokio::fs::create_dir_all(path).await?;
        }
        Ok(())
    }
}

/// Git-visible work found in a workspace directory that a delete would destroy.
/// Names the first repository found to be dirty, for logging.
#[derive(Debug, Clone, PartialEq, Eq)]
struct UnsavedWork {
    repo_dir_name: String,
    uncommitted: usize,
    untracked: usize,
}

#[derive(Debug, Clone)]
pub struct RepoWorkspaceInput {
    pub repo: Repo,
    pub target_branch: String,
    /// The Git directory that owns this repository's worktree administration
    /// *for this workspace*.
    ///
    /// For a workspace that runs on the coordinator this is `repo.path`, the
    /// operator's registered checkout, exactly as before. For a workspace placed
    /// on a worker it is the shared store, because `repo.path` names storage no
    /// other node can reach and a worktree created from it is unusable there.
    pub git_path: PathBuf,
}

impl RepoWorkspaceInput {
    /// Administer this repository's worktree in the registered checkout — the
    /// behaviour every workspace had before clustering, and the behaviour every
    /// coordinator-local workspace still has.
    pub fn new(repo: Repo, target_branch: String) -> Self {
        let git_path = repo.path.clone();
        Self {
            repo,
            target_branch,
            git_path,
        }
    }

    /// Administer this repository's worktree in the shared store, so the paths
    /// git records resolve identically on every node.
    pub fn shared(repo: Repo, target_branch: String, store: PathBuf) -> Self {
        Self {
            repo,
            target_branch,
            git_path: store,
        }
    }
}

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Repo(#[from] RepoError),
    #[error(transparent)]
    Worktree(#[from] WorktreeError),
    #[error(transparent)]
    GitService(#[from] GitServiceError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Workspace not found")]
    WorkspaceNotFound,
    #[error("Repository already attached to workspace")]
    RepoAlreadyAttached,
    #[error("Branch '{branch}' does not exist in repository '{repo_name}'")]
    BranchNotFound { repo_name: String, branch: String },
    #[error("No repositories provided")]
    NoRepositories,
    #[error("Partial workspace creation failed: {0}")]
    PartialCreation(String),
    #[error("shared workspace root must be absolute: {0}")]
    InvalidSharedRoot(PathBuf),
    /// A worktree that a worker must be able to use is backed by a repository
    /// the worker cannot resolve. Carries the repository name and a description
    /// of what the linkage actually resolved to, because the operator's next
    /// question is always "which repo, and pointing where?".
    #[error("worktree for repository '{repo_name}' is not usable from other nodes: {detail}")]
    WorktreeNotPortable { repo_name: String, detail: String },
    /// The shared repository store cannot serve a workspace's target branch.
    /// Carries the repository and the branch because this reaches the user
    /// through the API, and "which repo, which branch?" is the only question
    /// worth answering there — a generic internal error turns a one-line
    /// diagnosis into an investigation.
    #[error("shared repository store for '{repo_name}' cannot serve branch '{branch}': {detail}")]
    SharedStore {
        repo_name: String,
        branch: String,
        detail: String,
    },
}

/// Info about a single repo's worktree within a workspace
#[derive(Debug, Clone)]
pub struct RepoWorktree {
    pub repo_id: Uuid,
    pub repo_name: String,
    pub source_repo_path: PathBuf,
    pub worktree_path: PathBuf,
}

/// A container directory holding worktrees for all project repos
#[derive(Debug, Clone)]
pub struct WorktreeContainer {
    pub workspace_dir: PathBuf,
    pub worktrees: Vec<RepoWorktree>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceDeletionContext {
    pub workspace_id: Uuid,
    pub branch_name: String,
    pub workspace_dir: Option<PathBuf>,
    pub repositories: Vec<Repo>,
    pub repo_paths: Vec<PathBuf>,
    pub session_ids: Vec<Uuid>,
}

#[derive(Clone)]
pub struct ManagedWorkspace {
    pub workspace: DbWorkspace,
    pub repos: Vec<RepoWithTargetBranch>,
    db: DBService,
}

impl ManagedWorkspace {
    fn new(db: DBService, workspace: DbWorkspace, repos: Vec<RepoWithTargetBranch>) -> Self {
        Self {
            workspace,
            repos,
            db,
        }
    }

    async fn attach_repository(&self, repo: &WorkspaceRepoInput) -> Result<(), sqlx::Error> {
        let create_repo = CreateWorkspaceRepo {
            repo_id: repo.repo_id,
            target_branch: repo.target_branch.clone(),
        };

        WorkspaceRepo::create_many(
            &self.db.pool,
            self.workspace.id,
            std::slice::from_ref(&create_repo),
        )
        .await
        .map(|_| ())
    }

    async fn refresh(&mut self) -> Result<(), WorkspaceError> {
        self.workspace = DbWorkspace::find_by_id(&self.db.pool, self.workspace.id)
            .await?
            .ok_or(WorkspaceError::WorkspaceNotFound)?;
        self.repos = WorkspaceRepo::find_repos_with_target_branch_for_workspace(
            &self.db.pool,
            self.workspace.id,
        )
        .await?;
        Ok(())
    }

    pub async fn add_repository(
        &mut self,
        repo_ref: &WorkspaceRepoInput,
        git: &GitService,
    ) -> Result<(), WorkspaceError> {
        let repo = Repo::find_by_id(&self.db.pool, repo_ref.repo_id)
            .await?
            .ok_or(RepoError::NotFound)?;

        if !git.check_branch_exists(&repo.path, &repo_ref.target_branch)? {
            return Err(WorkspaceError::BranchNotFound {
                repo_name: repo.name,
                branch: repo_ref.target_branch.clone(),
            });
        }

        if WorkspaceRepo::find_by_workspace_and_repo_id(
            &self.db.pool,
            self.workspace.id,
            repo_ref.repo_id,
        )
        .await?
        .is_some()
        {
            return Err(WorkspaceError::RepoAlreadyAttached);
        }

        self.attach_repository(repo_ref).await?;
        self.refresh().await?;
        Ok(())
    }

    pub async fn associate_attachments(&self, attachment_ids: &[Uuid]) -> Result<(), sqlx::Error> {
        if attachment_ids.is_empty() {
            return Ok(());
        }

        WorkspaceAttachment::associate_many_dedup(&self.db.pool, self.workspace.id, attachment_ids)
            .await
    }

    pub async fn prepare_deletion_context(&self) -> Result<WorkspaceDeletionContext, sqlx::Error> {
        let repositories =
            WorkspaceRepo::find_repos_for_workspace(&self.db.pool, self.workspace.id).await?;
        let session_ids = Session::find_by_workspace_id(&self.db.pool, self.workspace.id)
            .await?
            .into_iter()
            .map(|session| session.id)
            .collect::<Vec<_>>();
        let repo_paths = repositories
            .iter()
            .map(|repo| repo.path.clone())
            .collect::<Vec<_>>();

        Ok(WorkspaceDeletionContext {
            workspace_id: self.workspace.id,
            branch_name: self.workspace.branch.clone(),
            workspace_dir: self.workspace.container_ref.clone().map(PathBuf::from),
            repositories,
            repo_paths,
            session_ids,
        })
    }

    pub async fn delete_record(&self) -> Result<u64, sqlx::Error> {
        DbWorkspace::delete(&self.db.pool, self.workspace.id).await
    }
}

#[derive(Clone)]
pub struct WorkspaceManager {
    db: DBService,
}

impl WorkspaceManager {
    pub fn new(db: DBService) -> Self {
        Self { db }
    }

    pub async fn load_managed_workspace(
        &self,
        workspace: DbWorkspace,
    ) -> Result<ManagedWorkspace, sqlx::Error> {
        let repos =
            WorkspaceRepo::find_repos_with_target_branch_for_workspace(&self.db.pool, workspace.id)
                .await?;
        Ok(ManagedWorkspace::new(self.db.clone(), workspace, repos))
    }

    pub fn spawn_workspace_deletion_cleanup(
        context: WorkspaceDeletionContext,
        delete_branches: bool,
    ) {
        tokio::spawn(async move {
            let WorkspaceDeletionContext {
                workspace_id,
                branch_name,
                workspace_dir,
                repositories,
                repo_paths,
                session_ids,
            } = context;

            for session_id in session_ids {
                if let Err(e) = Self::remove_session_process_logs(session_id).await {
                    warn!(
                        "Failed to remove filesystem process logs for session {}: {}",
                        session_id, e
                    );
                }
            }

            if let Some(workspace_dir) = workspace_dir {
                info!(
                    "Starting background cleanup for workspace {} at {}",
                    workspace_id,
                    workspace_dir.display()
                );

                // `repo_paths` is the deletion context's record of which Git
                // directory administers each repository's worktree, resolved by
                // whoever built the context. Pair them back up rather than
                // re-deriving from `repo.path`, which is wrong for a clustered
                // workspace.
                let cleanup_inputs: Vec<RepoWorkspaceInput> = repositories
                    .iter()
                    .cloned()
                    .zip(repo_paths.iter().cloned())
                    .map(|(repo, git_path)| RepoWorkspaceInput {
                        target_branch: String::new(),
                        repo,
                        git_path,
                    })
                    .collect();

                if let Err(e) = Self::cleanup_workspace(&workspace_dir, &cleanup_inputs).await {
                    error!(
                        "Background workspace cleanup failed for {} at {}: {}",
                        workspace_id,
                        workspace_dir.display(),
                        e
                    );
                } else {
                    info!(
                        "Background cleanup completed for workspace {}",
                        workspace_id
                    );
                }
            }

            if delete_branches {
                let git_service = GitService::new();
                for repo_path in repo_paths {
                    match git_service.delete_branch(&repo_path, &branch_name) {
                        Ok(()) => {
                            info!("Deleted branch '{}' from repo {:?}", branch_name, repo_path);
                        }
                        Err(e) => {
                            warn!(
                                "Failed to delete branch '{}' from repo {:?}: {}",
                                branch_name, repo_path, e
                            );
                        }
                    }
                }
            }
        });
    }

    async fn remove_session_process_logs(session_id: Uuid) -> Result<(), std::io::Error> {
        let dir = utils::execution_logs::process_logs_session_dir(session_id);
        match tokio::fs::remove_dir_all(&dir).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Create a workspace with worktrees for all repositories.
    /// On failure, rolls back any already-created worktrees.
    pub async fn create_workspace(
        workspace_dir: &Path,
        repos: &[RepoWorkspaceInput],
        branch_name: &str,
    ) -> Result<WorktreeContainer, WorkspaceError> {
        Self::create_workspace_with_locking(workspace_dir, repos, branch_name, None).await
    }

    pub async fn create_workspace_fenced(
        workspace_dir: &Path,
        repos: &[RepoWorkspaceInput],
        branch_name: &str,
        lock_manager: &RepositoryAdminLockManager,
    ) -> Result<WorktreeContainer, WorkspaceError> {
        Self::create_workspace_with_locking(workspace_dir, repos, branch_name, Some(lock_manager))
            .await
    }

    async fn create_workspace_with_locking(
        workspace_dir: &Path,
        repos: &[RepoWorkspaceInput],
        branch_name: &str,
        lock_manager: Option<&RepositoryAdminLockManager>,
    ) -> Result<WorktreeContainer, WorkspaceError> {
        if repos.is_empty() {
            return Err(WorkspaceError::NoRepositories);
        }

        info!(
            "Creating workspace at {} with {} repositories",
            workspace_dir.display(),
            repos.len()
        );

        tokio::fs::create_dir_all(workspace_dir).await?;

        let mut created_worktrees: Vec<RepoWorktree> = Vec::new();

        for input in repos {
            let worktree_path = workspace_dir.join(&input.repo.name);

            debug!(
                "Creating worktree for repo '{}' at {}",
                input.repo.name,
                worktree_path.display()
            );

            let create_result = match lock_manager {
                Some(lock_manager) => {
                    WorktreeManager::create_worktree_fenced(
                        lock_manager,
                        input.repo.id,
                        &input.git_path,
                        branch_name,
                        &worktree_path,
                        &input.target_branch,
                        true,
                    )
                    .await
                }
                None => {
                    WorktreeManager::create_worktree(
                        &input.git_path,
                        branch_name,
                        &worktree_path,
                        &input.target_branch,
                        true,
                    )
                    .await
                }
            };
            match create_result {
                Ok(()) => {
                    created_worktrees.push(RepoWorktree {
                        repo_id: input.repo.id,
                        repo_name: input.repo.name.clone(),
                        source_repo_path: input.git_path.clone(),
                        worktree_path,
                    });
                }
                Err(e) => {
                    error!(
                        "Failed to create worktree for repo '{}': {}. Rolling back...",
                        input.repo.name, e
                    );

                    // Rollback: cleanup all worktrees we've created so far
                    Self::cleanup_created_worktrees(&created_worktrees).await;

                    // Also remove the workspace directory if it's empty
                    if let Err(cleanup_err) = tokio::fs::remove_dir(workspace_dir).await {
                        debug!(
                            "Could not remove workspace dir during rollback: {}",
                            cleanup_err
                        );
                    }

                    return Err(WorkspaceError::PartialCreation(format!(
                        "Failed to create worktree for repo '{}': {}",
                        input.repo.name, e
                    )));
                }
            }
        }

        info!(
            "Successfully created workspace with {} worktrees",
            created_worktrees.len()
        );

        Ok(WorktreeContainer {
            workspace_dir: workspace_dir.to_path_buf(),
            worktrees: created_worktrees,
        })
    }

    /// Ensure all worktrees in a workspace exist (for cold restart scenarios)
    pub async fn ensure_workspace_exists(
        workspace_dir: &Path,
        repos: &[RepoWorkspaceInput],
        branch_name: &str,
    ) -> Result<(), WorkspaceError> {
        Self::ensure_workspace_exists_with_locking(workspace_dir, repos, branch_name, None).await
    }

    pub async fn ensure_workspace_exists_fenced(
        workspace_dir: &Path,
        repos: &[RepoWorkspaceInput],
        branch_name: &str,
        lock_manager: &RepositoryAdminLockManager,
    ) -> Result<(), WorkspaceError> {
        Self::ensure_workspace_exists_with_locking(
            workspace_dir,
            repos,
            branch_name,
            Some(lock_manager),
        )
        .await
    }

    async fn ensure_workspace_exists_with_locking(
        workspace_dir: &Path,
        repos: &[RepoWorkspaceInput],
        branch_name: &str,
        lock_manager: Option<&RepositoryAdminLockManager>,
    ) -> Result<(), WorkspaceError> {
        if repos.is_empty() {
            return Err(WorkspaceError::NoRepositories);
        }

        // Try legacy migration first (single repo projects only)
        // Old layout had worktree directly at workspace_dir; new layout has it at workspace_dir/{repo_name}
        if repos.len() == 1 && Self::migrate_legacy_worktree(workspace_dir, &repos[0]).await? {
            return Ok(());
        }

        if !workspace_dir.exists() {
            tokio::fs::create_dir_all(workspace_dir).await?;
        }

        let git = GitService::new();

        for input in repos {
            let repo = &input.repo;
            let worktree_path = workspace_dir.join(&repo.name);

            debug!(
                "Ensuring worktree exists for repo '{}' at {}",
                repo.name,
                worktree_path.display()
            );

            if git.check_branch_exists(&input.git_path, branch_name)? {
                match lock_manager {
                    Some(lock_manager) => {
                        WorktreeManager::ensure_worktree_exists_fenced(
                            lock_manager,
                            repo.id,
                            &input.git_path,
                            branch_name,
                            &worktree_path,
                        )
                        .await?;
                    }
                    None => {
                        WorktreeManager::ensure_worktree_exists(
                            &input.git_path,
                            branch_name,
                            &worktree_path,
                        )
                        .await?;
                    }
                }
            } else {
                info!(
                    "Workspace branch '{}' missing in repo '{}'; creating from target branch '{}'",
                    branch_name, repo.name, input.target_branch
                );
                match lock_manager {
                    Some(lock_manager) => {
                        WorktreeManager::create_worktree_fenced(
                            lock_manager,
                            repo.id,
                            &input.git_path,
                            branch_name,
                            &worktree_path,
                            &input.target_branch,
                            true,
                        )
                        .await?;
                    }
                    None => {
                        WorktreeManager::create_worktree(
                            &input.git_path,
                            branch_name,
                            &worktree_path,
                            &input.target_branch,
                            true,
                        )
                        .await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Clean up all worktrees in a workspace.
    ///
    /// Takes resolved inputs rather than bare `Repo` records, because a bare
    /// record cannot say which Git directory administers *this* workspace's
    /// worktrees. Cleaning a clustered workspace against `repo.path` would
    /// unregister nothing — the registration lives in the shared store — while
    /// still deleting the directory, leaving an orphaned registration behind.
    pub async fn cleanup_workspace(
        workspace_dir: &Path,
        repos: &[RepoWorkspaceInput],
    ) -> Result<(), WorkspaceError> {
        info!("Cleaning up workspace at {}", workspace_dir.display());

        let cleanup_data: Vec<WorktreeCleanup> = repos
            .iter()
            .map(|input| {
                let worktree_path = workspace_dir.join(&input.repo.name);
                WorktreeCleanup::new(worktree_path, Some(input.git_path.clone()))
            })
            .collect();

        WorktreeManager::batch_cleanup_worktrees(&cleanup_data).await?;

        // Remove the workspace directory itself
        if workspace_dir.exists() {
            tokio::fs::remove_dir_all(workspace_dir).await?;
        }

        Ok(())
    }

    /// Get the base directory for workspaces (same as worktree base dir)
    pub fn get_workspace_base_dir() -> PathBuf {
        WorktreeManager::get_worktree_base_dir()
    }

    /// Migrate a legacy single-worktree layout to the new workspace layout.
    /// Old layout: workspace_dir IS the worktree
    /// New layout: workspace_dir contains worktrees at workspace_dir/{repo_name}
    ///
    /// Returns Ok(true) if migration was performed, Ok(false) if no migration needed.
    async fn migrate_legacy_worktree(
        workspace_dir: &Path,
        input: &RepoWorkspaceInput,
    ) -> Result<bool, WorkspaceError> {
        let repo = &input.repo;
        let expected_worktree_path = workspace_dir.join(&repo.name);

        // Detect old-style: workspace_dir exists AND has .git file (worktree marker)
        // AND expected new location doesn't exist
        let git_file = workspace_dir.join(".git");
        let is_old_style = workspace_dir.exists()
            && git_file.exists()
            && git_file.is_file() // .git file = worktree, .git dir = main repo
            && !expected_worktree_path.exists();

        if !is_old_style {
            return Ok(false);
        }

        info!(
            "Detected legacy worktree at {}, migrating to new layout",
            workspace_dir.display()
        );

        // Move old worktree to temp location (can't move into subdirectory of itself)
        let temp_name = format!(
            "{}-migrating",
            workspace_dir
                .file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default()
        );
        let temp_path = workspace_dir.with_file_name(temp_name);

        WorktreeManager::move_worktree(&input.git_path, workspace_dir, &temp_path).await?;

        // Create new workspace directory
        tokio::fs::create_dir_all(workspace_dir).await?;

        // Move worktree to final location using git worktree move
        WorktreeManager::move_worktree(&input.git_path, &temp_path, &expected_worktree_path)
            .await?;

        if temp_path.exists() {
            let _ = tokio::fs::remove_dir_all(&temp_path).await;
        }

        info!(
            "Successfully migrated legacy worktree to {}",
            expected_worktree_path.display()
        );

        Ok(true)
    }

    /// Helper to cleanup worktrees during rollback
    async fn cleanup_created_worktrees(worktrees: &[RepoWorktree]) {
        for worktree in worktrees {
            let cleanup = WorktreeCleanup::new(
                worktree.worktree_path.clone(),
                Some(worktree.source_repo_path.clone()),
            );

            if let Err(e) = WorktreeManager::cleanup_worktree(&cleanup).await {
                error!(
                    "Failed to cleanup worktree '{}' during rollback: {}",
                    worktree.repo_name, e
                );
            }
        }
    }

    pub async fn cleanup_orphan_workspaces(&self, allow_reclamation: bool) {
        if std::env::var("DISABLE_WORKTREE_CLEANUP").is_ok() {
            info!(
                "Orphan workspace cleanup is disabled via DISABLE_WORKTREE_CLEANUP environment variable"
            );
            return;
        }
        if !allow_reclamation {
            info!(
                "Orphan workspace cleanup is retaining unreferenced directories because shared workers may still own them"
            );
            return;
        }

        // Always clean up the default directory
        let default_dir = WorktreeManager::get_default_worktree_base_dir();
        self.cleanup_orphans_in_directory(&default_dir).await;

        // Also clean up custom directory if it's different from the default
        let current_dir = Self::get_workspace_base_dir();
        if current_dir != default_dir {
            self.cleanup_orphans_in_directory(&current_dir).await;
        }
    }

    async fn cleanup_orphans_in_directory(&self, workspace_base_dir: &Path) {
        if !workspace_base_dir.exists() {
            debug!(
                "Workspace base directory {} does not exist, skipping orphan cleanup",
                workspace_base_dir.display()
            );
            return;
        }

        let entries = match std::fs::read_dir(workspace_base_dir) {
            Ok(entries) => entries,
            Err(e) => {
                error!(
                    "Failed to read workspace base directory {}: {}",
                    workspace_base_dir.display(),
                    e
                );
                return;
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    warn!("Failed to read directory entry: {}", e);
                    continue;
                }
            };

            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let workspace_path_str = path.to_string_lossy().to_string();
            if let Ok(false) =
                DbWorkspace::container_ref_exists(&self.db.pool, &workspace_path_str).await
            {
                // Uncommitted work is irreplaceable, so establish that this
                // directory holds none before deleting it. Orphan status is
                // decided by an exact string match on `container_ref`, so any
                // path drift (a symlinked base dir, a changed workspace_dir
                // override) can misclassify a live workspace as abandoned;
                // this check is what stops that from costing a session's work.
                match Self::workspace_dir_unsaved_work(&path) {
                    Ok(None) => {}
                    Ok(Some(work)) => {
                        info!(
                            "Retaining workspace {}: no workspace record references it, but it \
                             holds unsaved work ({} uncommitted, {} untracked in '{}')",
                            workspace_path_str,
                            work.uncommitted,
                            work.untracked,
                            work.repo_dir_name
                        );
                        continue;
                    }
                    Err(e) => {
                        // An error is not evidence of emptiness. Retain, and
                        // keep sweeping the remaining candidates: one
                        // unreadable directory must not disable cleanup of
                        // every other directory in the base dir.
                        warn!(
                            "Retaining workspace {}: no workspace record references it, but its \
                             cleanliness could not be determined: {}",
                            workspace_path_str, e
                        );
                        continue;
                    }
                }

                info!(
                    "Removing orphaned workspace {}: no workspace record references it and no \
                     unsaved work was found",
                    workspace_path_str
                );
                if let Err(e) = Self::cleanup_workspace_without_repos(&path).await {
                    error!(
                        "Failed to remove orphaned workspace {}: {}",
                        workspace_path_str, e
                    );
                } else {
                    info!(
                        "Successfully removed orphaned workspace: {}",
                        workspace_path_str
                    );
                }
            }
        }
    }

    /// Unsaved work found in an orphan-candidate workspace directory.
    fn workspace_dir_unsaved_work(
        workspace_dir: &Path,
    ) -> Result<Option<UnsavedWork>, WorkspaceError> {
        let entries = std::fs::read_dir(workspace_dir)?;
        let git = GitService::new();

        for entry in entries {
            // Propagate instead of skipping. `filter_map(|e| e.ok())` would
            // silently treat an unreadable entry as absent, and `Path::exists`
            // returns false both for "not there" and "could not tell" — either
            // would turn an indeterminate repo into a deletable one.
            let repo_dir = entry?.path();
            if !std::fs::metadata(&repo_dir)?.is_dir() {
                continue;
            }
            // Worktrees carry a `.git` file; a plain clone carries a `.git`
            // directory. Accept either — both hold work worth keeping.
            // `try_exists` distinguishes a genuine absence from a failed stat.
            if !repo_dir.join(".git").try_exists()? {
                continue;
            }

            // A probe failure is indeterminate, never "clean": propagate so the
            // caller retains the directory.
            let (uncommitted, untracked) = git.get_worktree_change_counts(&repo_dir)?;
            if uncommitted > 0 || untracked > 0 {
                return Ok(Some(UnsavedWork {
                    repo_dir_name: repo_dir
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    uncommitted,
                    untracked,
                }));
            }
        }

        // Either every repo checked out clean, or the directory contains no git
        // repositories at all. The latter matters: without it, retain-on-error
        // would leak every unprobeable directory forever and orphan cleanup
        // would stop reclaiming anything.
        Ok(None)
    }

    async fn cleanup_workspace_without_repos(workspace_dir: &Path) -> Result<(), WorkspaceError> {
        info!(
            "Cleaning up orphaned workspace at {}",
            workspace_dir.display()
        );

        let entries = match std::fs::read_dir(workspace_dir) {
            Ok(entries) => entries,
            Err(e) => {
                debug!(
                    "Cannot read workspace directory {}, attempting direct removal: {}",
                    workspace_dir.display(),
                    e
                );
                return tokio::fs::remove_dir_all(workspace_dir)
                    .await
                    .map_err(WorkspaceError::Io);
            }
        };

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir()
                && let Err(e) = WorktreeManager::cleanup_suspected_worktree(&path).await
            {
                warn!("Failed to cleanup suspected worktree: {}", e);
            }
        }

        // Propagate rather than swallow: returning Ok(()) here made the caller
        // log "Successfully removed orphaned workspace" for a directory that is
        // still on disk.
        if workspace_dir.exists() {
            tokio::fs::remove_dir_all(workspace_dir)
                .await
                .map_err(WorkspaceError::Io)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod shared_path_tests {
    use std::path::PathBuf;

    use uuid::Uuid;

    use super::SharedWorkspacePaths;

    #[test]
    fn derives_stable_id_only_paths_under_shared_root() {
        let paths = SharedWorkspacePaths::new("/srv/vibe-kanban-shared").unwrap();
        let repo_id = Uuid::from_u128(1);
        let workspace_id = Uuid::from_u128(2);
        let execution_id = Uuid::from_u128(3);

        assert_eq!(
            paths.repository_dir(repo_id),
            PathBuf::from("/srv/vibe-kanban-shared/repositories").join(repo_id.to_string())
        );
        assert_eq!(
            paths.workspace_dir(workspace_id),
            PathBuf::from("/srv/vibe-kanban-shared/workspaces").join(workspace_id.to_string())
        );
        assert_eq!(
            paths.execution_log_dir(execution_id),
            PathBuf::from("/srv/vibe-kanban-shared/execution-logs").join(execution_id.to_string())
        );
    }

    #[test]
    fn rejects_relative_shared_root() {
        assert!(SharedWorkspacePaths::new("relative/shared").is_err());
    }

    #[tokio::test]
    async fn creates_all_base_directories() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("shared");
        let paths = SharedWorkspacePaths::new(&root).unwrap();
        paths.create_base_dirs().await.unwrap();
        assert!(paths.repositories_dir().is_dir());
        assert!(paths.workspaces_dir().is_dir());
        assert!(paths.execution_logs_dir().is_dir());
    }
}

#[cfg(test)]
mod orphan_cleanup_tests {
    use std::{fs, path::Path, process::Command};

    use super::{UnsavedWork, WorkspaceManager};

    fn git(dir: &Path, args: &[&str]) {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git should be available on PATH");
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// A repo with one committed file, as a plain clone (a `.git` directory).
    /// The probe accepts either marker shape, and this keeps fixtures cheap.
    fn init_repo(parent: &Path, name: &str) -> std::path::PathBuf {
        let repo = parent.join(name);
        fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "--initial-branch=main"]);
        git(&repo, &["config", "user.email", "test@example.com"]);
        git(&repo, &["config", "user.name", "Test"]);
        fs::write(repo.join("tracked.txt"), "base\n").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-m", "base"]);
        repo
    }

    fn unsaved(dir: &Path) -> Option<UnsavedWork> {
        WorkspaceManager::workspace_dir_unsaved_work(dir).expect("probe should succeed")
    }

    #[test]
    fn clean_workspace_reports_no_unsaved_work() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path(), "repo-a");
        assert_eq!(unsaved(tmp.path()), None);
    }

    #[test]
    fn directory_without_git_repos_reports_no_unsaved_work() {
        // Bounds the disk leak: retain-on-error must not mean retain-forever
        // for directories that hold no git work at all.
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("not-a-repo")).unwrap();
        fs::write(tmp.path().join("not-a-repo/file.txt"), "x").unwrap();
        assert_eq!(unsaved(tmp.path()), None);
    }

    #[test]
    fn modified_tracked_file_is_unsaved_work() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo(tmp.path(), "repo-a");
        fs::write(repo.join("tracked.txt"), "modified\n").unwrap();

        let work = unsaved(tmp.path()).expect("dirty repo must be retained");
        assert_eq!(work.repo_dir_name, "repo-a");
        assert_eq!(work.uncommitted, 1);
    }

    #[test]
    fn untracked_file_is_unsaved_work() {
        // The reported incident lost new files that were never committed.
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo(tmp.path(), "repo-a");
        fs::write(repo.join("brand-new.md"), "new\n").unwrap();

        let work = unsaved(tmp.path()).expect("untracked files must be retained");
        assert_eq!(work.untracked, 1);
    }

    #[test]
    fn staged_but_uncommitted_file_is_unsaved_work() {
        // Staged changes were lost too, and the codebase's two cleanliness
        // helpers disagree about them — pin the behaviour we rely on here.
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo(tmp.path(), "repo-a");
        fs::write(repo.join("staged.md"), "staged\n").unwrap();
        git(&repo, &["add", "staged.md"]);

        let work = unsaved(tmp.path()).expect("staged changes must be retained");
        assert_eq!(work.uncommitted, 1);
        assert_eq!(work.untracked, 0);
    }

    #[test]
    fn dirty_second_repo_is_found_in_multi_repo_workspace() {
        // container_ref is the parent of N repos; a clean first repo must not
        // mask unsaved work in a later one.
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path(), "repo-a");
        let repo_b = init_repo(tmp.path(), "repo-b");
        fs::write(repo_b.join("tracked.txt"), "changed in b\n").unwrap();

        let work = unsaved(tmp.path()).expect("dirty second repo must be retained");
        assert_eq!(work.repo_dir_name, "repo-b");
        assert_eq!(work.uncommitted, 1);
    }

    #[test]
    fn unreadable_workspace_dir_is_indeterminate_not_clean() {
        // An error must never be reported as "nothing here to lose".
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("does-not-exist");
        assert!(WorkspaceManager::workspace_dir_unsaved_work(&missing).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_repo_subdir_is_indeterminate_not_clean() {
        // Found by independent review: skipping entries that cannot be stat'ed
        // would let an otherwise-clean-looking workspace be deleted even though
        // one of its repos was never actually inspected.
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path(), "repo-a"); // clean, so only the opaque dir decides
        let opaque = tmp.path().join("repo-b");
        fs::create_dir_all(opaque.join("inner")).unwrap();
        fs::set_permissions(&opaque, fs::Permissions::from_mode(0o000)).unwrap();

        let result = WorkspaceManager::workspace_dir_unsaved_work(tmp.path());

        // Restore before asserting so the tempdir can always be cleaned up.
        fs::set_permissions(&opaque, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            result.is_err(),
            "an unreadable repo subdir must be indeterminate, not treated as clean"
        );
    }
}
