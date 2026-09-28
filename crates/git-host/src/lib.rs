mod detection;
mod types;

pub mod azure;
pub mod github;

use std::path::Path;

use async_trait::async_trait;
use detection::detect_provider_from_url;
use enum_dispatch::enum_dispatch;
pub use types::{
    CheckSource, ChecksOverall, CreatePrRequest, GitHostError, MergeMethod, MergeOutcome,
    MergeRefusal, PrCheck, PrChecks, PrComment, PrCommentAuthor, PrReference, PrReviewComment,
    PrState, ProviderKind, PullRequestDetail, ReviewCommentUser, SourceCoverage, SourceRead,
    SourceState, UnifiedPrComment, UpdatePrFields, aggregate_checks, merge_gate,
    parse_pr_reference, same_repo,
};

use self::{azure::AzureDevOpsProvider, github::GitHubProvider};

#[async_trait]
#[enum_dispatch(GitHostService)]
pub trait GitHostProvider: Send + Sync {
    async fn create_pr(
        &self,
        repo_path: &Path,
        remote_url: &str,
        request: &CreatePrRequest,
    ) -> Result<PullRequestDetail, GitHostError>;

    async fn get_pr_status(&self, pr_url: &str) -> Result<PullRequestDetail, GitHostError>;

    async fn list_prs_for_branch(
        &self,
        repo_path: &Path,
        remote_url: &str,
        branch_name: &str,
    ) -> Result<Vec<PullRequestDetail>, GitHostError>;

    async fn get_pr_comments(
        &self,
        repo_path: &Path,
        remote_url: &str,
        pr_number: i64,
    ) -> Result<Vec<UnifiedPrComment>, GitHostError>;

    async fn list_open_prs(
        &self,
        repo_path: &Path,
        remote_url: &str,
    ) -> Result<Vec<PullRequestDetail>, GitHostError>;

    fn provider_kind(&self) -> ProviderKind;

    // PR management. Providers without an implementation report
    // `UnsupportedProvider` rather than failing in a provider-specific way.

    /// Canonical `(owner, repo)` of `remote_url`, used to refuse PR URLs that
    /// belong to another repository.
    async fn repo_identity(
        &self,
        _repo_path: &Path,
        _remote_url: &str,
    ) -> Result<(String, String), GitHostError> {
        Err(GitHostError::UnsupportedProvider)
    }

    async fn get_pr_state(
        &self,
        _repo_path: &Path,
        _remote_url: &str,
        _number: i64,
    ) -> Result<PrState, GitHostError> {
        Err(GitHostError::UnsupportedProvider)
    }

    /// Checks for `head_sha`, reporting per-source coverage.
    async fn list_pr_checks(
        &self,
        _repo_path: &Path,
        _remote_url: &str,
        _head_sha: &str,
    ) -> Result<PrChecks, GitHostError> {
        Err(GitHostError::UnsupportedProvider)
    }

    /// Merge exactly `head_sha`. `delete_branch` names the remote head branch
    /// to delete afterwards (best effort, reported in the outcome).
    async fn merge_pr(
        &self,
        _repo_path: &Path,
        _remote_url: &str,
        _number: i64,
        _method: MergeMethod,
        _head_sha: &str,
        _delete_branch: Option<&str>,
    ) -> Result<MergeOutcome, GitHostError> {
        Err(GitHostError::UnsupportedProvider)
    }

    async fn update_pr(
        &self,
        _repo_path: &Path,
        _remote_url: &str,
        _number: i64,
        _fields: &UpdatePrFields,
    ) -> Result<(), GitHostError> {
        Err(GitHostError::UnsupportedProvider)
    }
}

#[enum_dispatch]
pub enum GitHostService {
    GitHub(GitHubProvider),
    AzureDevOps(AzureDevOpsProvider),
}

impl GitHostService {
    pub fn from_url(url: &str) -> Result<Self, GitHostError> {
        match detect_provider_from_url(url) {
            ProviderKind::GitHub => Ok(Self::GitHub(GitHubProvider::new()?)),
            ProviderKind::AzureDevOps => Ok(Self::AzureDevOps(AzureDevOpsProvider::new()?)),
            ProviderKind::Unknown => Err(GitHostError::UnsupportedProvider),
        }
    }
}
