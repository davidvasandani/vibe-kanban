mod shared_repository;
mod workspace_manager;

pub use shared_repository::{AdoptOutcome, SharedRepositoryStore};
pub use workspace_manager::{
    ManagedWorkspace, RepoWorkspaceInput, RepoWorktree, SharedWorkspacePaths,
    WorkspaceDeletionContext, WorkspaceError, WorkspaceManager, WorktreeContainer,
};
