//! GitHub hosting service implementation.

mod cli;

use std::{path::Path, time::Duration};

use async_trait::async_trait;
use backon::{ExponentialBuilder, Retryable};
pub use cli::GhCli;
use cli::{CheckPage, GhCliError, GitHubRepoInfo};
use tokio::task;
use tracing::info;

use crate::{
    GitHostProvider,
    types::{
        CreatePrRequest, GitHostError, MergeMethod, MergeOutcome, PrChecks, PrComment,
        PrReviewComment, PrState, ProviderKind, PullRequestDetail, SourceRead, UnifiedPrComment,
        UpdatePrFields, aggregate_checks,
    },
};

#[derive(Debug, Clone)]
pub struct GitHubProvider {
    gh_cli: GhCli,
}

impl GitHubProvider {
    pub fn new() -> Result<Self, GitHostError> {
        Ok(Self {
            gh_cli: GhCli::new(),
        })
    }

    async fn get_repo_info(
        &self,
        remote_url: &str,
        repo_path: &Path,
    ) -> Result<GitHubRepoInfo, GitHostError> {
        let cli = self.gh_cli.clone();
        let url = remote_url.to_string();
        let path = repo_path.to_path_buf();
        task::spawn_blocking(move || cli.get_repo_info(&url, &path))
            .await
            .map_err(|err| {
                GitHostError::Repository(format!("Failed to get repo info from URL: {err}"))
            })?
            .map_err(Into::into)
    }

    async fn fetch_general_comments(
        &self,
        cli: &GhCli,
        repo_info: &GitHubRepoInfo,
        pr_number: i64,
    ) -> Result<Vec<PrComment>, GitHostError> {
        let cli = cli.clone();
        let repo_info = repo_info.clone();

        (|| async {
            let cli = cli.clone();
            let repo_info = repo_info.clone();

            let comments = task::spawn_blocking(move || cli.get_pr_comments(&repo_info, pr_number))
                .await
                .map_err(|err| {
                    GitHostError::PullRequest(format!(
                        "Failed to execute GitHub CLI for fetching PR comments: {err}"
                    ))
                })?;
            comments.map_err(GitHostError::from)
        })
        .retry(
            &ExponentialBuilder::default()
                .with_min_delay(Duration::from_secs(1))
                .with_max_delay(Duration::from_secs(30))
                .with_max_times(3)
                .with_jitter(),
        )
        .when(|e: &GitHostError| e.should_retry())
        .notify(|err: &GitHostError, dur: Duration| {
            tracing::warn!(
                "GitHub API call failed, retrying after {:.2}s: {}",
                dur.as_secs_f64(),
                err
            );
        })
        .await
    }

    async fn fetch_review_comments(
        &self,
        cli: &GhCli,
        repo_info: &GitHubRepoInfo,
        pr_number: i64,
    ) -> Result<Vec<PrReviewComment>, GitHostError> {
        let cli = cli.clone();
        let repo_info = repo_info.clone();

        (|| async {
            let cli = cli.clone();
            let repo_info = repo_info.clone();

            let comments =
                task::spawn_blocking(move || cli.get_pr_review_comments(&repo_info, pr_number))
                    .await
                    .map_err(|err| {
                        GitHostError::PullRequest(format!(
                            "Failed to execute GitHub CLI for fetching review comments: {err}"
                        ))
                    })?;
            comments.map_err(GitHostError::from)
        })
        .retry(
            &ExponentialBuilder::default()
                .with_min_delay(Duration::from_secs(1))
                .with_max_delay(Duration::from_secs(30))
                .with_max_times(3)
                .with_jitter(),
        )
        .when(|e: &GitHostError| e.should_retry())
        .notify(|err: &GitHostError, dur: Duration| {
            tracing::warn!(
                "GitHub API call failed, retrying after {:.2}s: {}",
                dur.as_secs_f64(),
                err
            );
        })
        .await
    }
}

impl From<GhCliError> for GitHostError {
    fn from(error: GhCliError) -> Self {
        match &error {
            GhCliError::AuthFailed(msg) => GitHostError::AuthFailed(msg.clone()),
            GhCliError::NotAvailable => GitHostError::CliNotInstalled {
                provider: ProviderKind::GitHub,
            },
            GhCliError::CommandFailed(msg) => {
                let lower = msg.to_ascii_lowercase();
                if lower.contains("403")
                    || lower.contains("forbidden")
                    || lower.contains("resource not accessible")
                {
                    GitHostError::InsufficientPermissions(msg.clone())
                } else if lower.contains("404") || lower.contains("not found") {
                    GitHostError::RepoNotFoundOrNoAccess(msg.clone())
                } else if lower.contains("not a git repository") {
                    GitHostError::NotAGitRepository(msg.clone())
                } else {
                    GitHostError::PullRequest(msg.clone())
                }
            }
            GhCliError::UnexpectedOutput(msg) => GitHostError::UnexpectedOutput(msg.clone()),
        }
    }
}

#[async_trait]
impl GitHostProvider for GitHubProvider {
    async fn create_pr(
        &self,
        repo_path: &Path,
        remote_url: &str,
        request: &CreatePrRequest,
    ) -> Result<PullRequestDetail, GitHostError> {
        // Get owner/repo from the remote URL (target repo for the PR).
        let target_repo_info = self.get_repo_info(remote_url, repo_path).await?;

        // For cross-fork PRs, get the head repo info to format head_branch as "owner:branch".
        let head_branch = if let Some(head_url) = &request.head_repo_url {
            let head_repo_info = self.get_repo_info(head_url, repo_path).await?;
            if head_repo_info.owner != target_repo_info.owner {
                format!("{}:{}", head_repo_info.owner, request.head_branch)
            } else {
                request.head_branch.clone()
            }
        } else {
            request.head_branch.clone()
        };

        let mut request_clone = request.clone();
        request_clone.head_branch = head_branch;

        (|| async {
            let cli = self.gh_cli.clone();
            let request = request_clone.clone();
            let target_repo = target_repo_info.clone();
            let repo_path = repo_path.to_path_buf();

            let cli_result =
                task::spawn_blocking(move || cli.create_pr(&request, &target_repo, &repo_path))
                    .await
                    .map_err(|err| {
                        GitHostError::PullRequest(format!(
                            "Failed to execute GitHub CLI for PR creation: {err}"
                        ))
                    })?
                    .map_err(GitHostError::from)?;

            info!(
                "Created GitHub PR #{} for branch {}",
                cli_result.number, request_clone.head_branch
            );

            Ok(cli_result)
        })
        .retry(
            &ExponentialBuilder::default()
                .with_min_delay(Duration::from_secs(1))
                .with_max_delay(Duration::from_secs(30))
                .with_max_times(3)
                .with_jitter(),
        )
        .when(|e: &GitHostError| e.should_retry())
        .notify(|err: &GitHostError, dur: Duration| {
            tracing::warn!(
                "GitHub API call failed, retrying after {:.2}s: {}",
                dur.as_secs_f64(),
                err
            );
        })
        .await
    }

    async fn get_pr_status(&self, pr_url: &str) -> Result<PullRequestDetail, GitHostError> {
        let cli = self.gh_cli.clone();
        let url = pr_url.to_string();

        (|| async {
            let cli = cli.clone();
            let url = url.clone();
            let pr = task::spawn_blocking(move || cli.view_pr(&url))
                .await
                .map_err(|err| {
                    GitHostError::PullRequest(format!(
                        "Failed to execute GitHub CLI for viewing PR: {err}"
                    ))
                })?;
            pr.map_err(GitHostError::from)
        })
        .retry(
            &ExponentialBuilder::default()
                .with_min_delay(Duration::from_secs(1))
                .with_max_delay(Duration::from_secs(30))
                .with_max_times(3)
                .with_jitter(),
        )
        .when(|err: &GitHostError| err.should_retry())
        .notify(|err: &GitHostError, dur: Duration| {
            tracing::warn!(
                "GitHub API call failed, retrying after {:.2}s: {}",
                dur.as_secs_f64(),
                err
            );
        })
        .await
    }

    async fn list_prs_for_branch(
        &self,
        repo_path: &Path,
        remote_url: &str,
        branch_name: &str,
    ) -> Result<Vec<PullRequestDetail>, GitHostError> {
        let repo_info = self.get_repo_info(remote_url, repo_path).await?;

        let cli = self.gh_cli.clone();
        let branch = branch_name.to_string();

        (|| async {
            let cli = cli.clone();
            let repo_info = repo_info.clone();
            let branch = branch.clone();

            let prs = task::spawn_blocking(move || cli.list_prs_for_branch(&repo_info, &branch))
                .await
                .map_err(|err| {
                    GitHostError::PullRequest(format!(
                        "Failed to execute GitHub CLI for listing PRs: {err}"
                    ))
                })?;
            prs.map_err(GitHostError::from)
        })
        .retry(
            &ExponentialBuilder::default()
                .with_min_delay(Duration::from_secs(1))
                .with_max_delay(Duration::from_secs(30))
                .with_max_times(3)
                .with_jitter(),
        )
        .when(|e: &GitHostError| e.should_retry())
        .notify(|err: &GitHostError, dur: Duration| {
            tracing::warn!(
                "GitHub API call failed, retrying after {:.2}s: {}",
                dur.as_secs_f64(),
                err
            );
        })
        .await
    }

    async fn get_pr_comments(
        &self,
        repo_path: &Path,
        remote_url: &str,
        pr_number: i64,
    ) -> Result<Vec<UnifiedPrComment>, GitHostError> {
        let repo_info = self.get_repo_info(remote_url, repo_path).await?;

        // Fetch both types of comments in parallel
        let cli1 = self.gh_cli.clone();
        let cli2 = self.gh_cli.clone();

        let (general_result, review_result) = tokio::join!(
            self.fetch_general_comments(&cli1, &repo_info, pr_number),
            self.fetch_review_comments(&cli2, &repo_info, pr_number)
        );

        let general_comments = general_result?;
        let review_comments = review_result?;

        // Convert and merge into unified timeline
        let mut unified: Vec<UnifiedPrComment> = Vec::new();

        for c in general_comments {
            unified.push(UnifiedPrComment::General {
                id: c.id,
                author: c.author.login,
                author_association: Some(c.author_association),
                body: c.body,
                created_at: c.created_at,
                url: Some(c.url),
            });
        }

        for c in review_comments {
            unified.push(UnifiedPrComment::Review {
                id: c.id,
                author: c.user.login,
                author_association: Some(c.author_association),
                body: c.body,
                created_at: c.created_at,
                url: Some(c.html_url),
                path: c.path,
                line: c.line,
                side: c.side,
                diff_hunk: Some(c.diff_hunk),
            });
        }

        // Sort by creation time
        unified.sort_by_key(|c| c.created_at());

        Ok(unified)
    }

    async fn list_open_prs(
        &self,
        repo_path: &Path,
        remote_url: &str,
    ) -> Result<Vec<PullRequestDetail>, GitHostError> {
        let repo_info = self.get_repo_info(remote_url, repo_path).await?;

        let cli = self.gh_cli.clone();

        (|| async {
            let cli = cli.clone();
            let owner = repo_info.owner.clone();
            let repo_name = repo_info.repo_name.clone();

            let prs = task::spawn_blocking(move || cli.list_prs(&owner, &repo_name))
                .await
                .map_err(|err| {
                    GitHostError::PullRequest(format!(
                        "Failed to execute GitHub CLI for listing PRs: {err}"
                    ))
                })?;
            prs.map_err(GitHostError::from)
        })
        .retry(
            &ExponentialBuilder::default()
                .with_min_delay(Duration::from_secs(1))
                .with_max_delay(Duration::from_secs(30))
                .with_max_times(3)
                .with_jitter(),
        )
        .when(|e: &GitHostError| e.should_retry())
        .notify(|err: &GitHostError, dur: Duration| {
            tracing::warn!(
                "GitHub API call failed, retrying after {:.2}s: {}",
                dur.as_secs_f64(),
                err
            );
        })
        .await
    }

    fn provider_kind(&self) -> ProviderKind {
        ProviderKind::GitHub
    }

    async fn repo_identity(
        &self,
        repo_path: &Path,
        remote_url: &str,
    ) -> Result<(String, String, String), GitHostError> {
        let info = self.get_repo_info(remote_url, repo_path).await?;
        let host = info
            .hostname
            .clone()
            .unwrap_or_else(|| "github.com".to_string());
        Ok((host, info.owner, info.repo_name))
    }

    async fn get_pr_state(
        &self,
        repo_path: &Path,
        remote_url: &str,
        number: i64,
    ) -> Result<PrState, GitHostError> {
        let info = self.get_repo_info(remote_url, repo_path).await?;
        let (cli, path) = (self.gh_cli.clone(), repo_path.to_path_buf());
        blocking("reading the PR", move || {
            cli.get_pr_state(&info, number, &path)
        })
        .await
    }

    async fn list_pr_checks(
        &self,
        repo_path: &Path,
        remote_url: &str,
        head_sha: &str,
    ) -> Result<PrChecks, GitHostError> {
        let info = self.get_repo_info(remote_url, repo_path).await?;
        let read =
            |fetch: fn(&GhCli, &GitHubRepoInfo, &str, &Path) -> Result<CheckPage, GhCliError>| {
                let (cli, info, sha, path) = (
                    self.gh_cli.clone(),
                    info.clone(),
                    head_sha.to_string(),
                    repo_path.to_path_buf(),
                );
                async move {
                    source_read(
                        blocking("reading checks", move || fetch(&cli, &info, &sha, &path)).await,
                    )
                }
            };
        let check_runs = read(GhCli::list_check_runs).await;
        let statuses = read(GhCli::get_combined_status).await;
        // Actions jobs only matter when check runs are unreadable (they are a
        // subset of check runs), so skip the extra calls otherwise.
        let actions_jobs = if matches!(check_runs, SourceRead::Ok { .. }) {
            SourceRead::Error("not read: check runs were readable".to_string())
        } else {
            read(GhCli::list_actions_jobs).await
        };
        Ok(aggregate_checks(
            head_sha,
            check_runs,
            statuses,
            actions_jobs,
        ))
    }

    async fn merge_pr(
        &self,
        repo_path: &Path,
        remote_url: &str,
        number: i64,
        method: MergeMethod,
        head_sha: &str,
        delete_branch: Option<&str>,
    ) -> Result<MergeOutcome, GitHostError> {
        let info = self.get_repo_info(remote_url, repo_path).await?;
        let (cli, merge_info, sha, path) = (
            self.gh_cli.clone(),
            info.clone(),
            head_sha.to_string(),
            repo_path.to_path_buf(),
        );
        // Not retried: a merge is not idempotent from the caller's view, and
        // the head-SHA guard already turns a race into a clear refusal.
        let mut outcome = blocking("merging the PR", move || {
            cli.merge_pr(&merge_info, number, method, &sha, &path)
        })
        .await?;
        if let Some(branch) = delete_branch.filter(|_| outcome.merged) {
            let (cli, branch, path) = (
                self.gh_cli.clone(),
                branch.to_string(),
                repo_path.to_path_buf(),
            );
            match blocking("deleting the branch", move || {
                cli.delete_remote_branch(&info, &branch, &path)
            })
            .await
            {
                Ok(()) => outcome.branch_deleted = Some(true),
                Err(error) => {
                    outcome.branch_deleted = Some(false);
                    outcome.branch_delete_error = Some(error.to_string());
                }
            }
        }
        Ok(outcome)
    }

    async fn update_pr(
        &self,
        repo_path: &Path,
        remote_url: &str,
        number: i64,
        fields: &UpdatePrFields,
    ) -> Result<(), GitHostError> {
        let info = self.get_repo_info(remote_url, repo_path).await?;
        if fields.title.is_some() || fields.body.is_some() {
            let (cli, info, path) = (self.gh_cli.clone(), info.clone(), repo_path.to_path_buf());
            let (title, body) = (fields.title.clone(), fields.body.clone());
            blocking("editing the PR", move || {
                cli.patch_pr(&info, number, title.as_deref(), body.as_deref(), &path)
            })
            .await?;
        }
        if let Some(ready) = fields.ready_for_review {
            let (cli, path) = (self.gh_cli.clone(), repo_path.to_path_buf());
            blocking("changing draft state", move || {
                cli.set_pr_ready(&info, number, ready, &path)
            })
            .await?;
        }
        Ok(())
    }
}

/// Run a blocking `gh` call off the async runtime and map its error.
async fn blocking<T: Send + 'static>(
    what: &str,
    call: impl FnOnce() -> Result<T, GhCliError> + Send + 'static,
) -> Result<T, GitHostError> {
    task::spawn_blocking(call)
        .await
        .map_err(|err| {
            GitHostError::PullRequest(format!("Failed to run the GitHub CLI for {what}: {err}"))
        })?
        .map_err(GitHostError::from)
}

/// A 403 on one check source is a coverage fact, not a failure of the call.
fn source_read(result: Result<CheckPage, GitHostError>) -> SourceRead {
    match result {
        Ok((checks, truncated)) => SourceRead::Ok { checks, truncated },
        Err(GitHostError::InsufficientPermissions(detail)) => SourceRead::Forbidden(detail),
        Err(error) => SourceRead::Error(error.to_string()),
    }
}
