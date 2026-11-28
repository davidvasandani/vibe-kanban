mod shared_repository;
pub mod source_mirror;
mod workspace_manager;

pub use shared_repository::{AdoptOutcome, SharedRepositoryStore, SweepReport};
pub use source_mirror::{
    RequestOutcome, SourceMirror, SourceMirrorConfig, SourceMirrorFailure, SourceMirrorFailureKind,
};
pub use workspace_manager::{
    ManagedWorkspace, RepoWorkspaceInput, RepoWorktree, SharedWorkspacePaths,
    WorkspaceDeletionContext, WorkspaceError, WorkspaceManager, WorktreeContainer,
    is_target_branch,
};
