//! Minimal helpers around the GitHub CLI (`gh`).
//!
//! This module provides low-level access to the GitHub CLI for operations
//! the REST client does not cover well.

use std::{
    ffi::{OsStr, OsString},
    io::Write,
    path::Path,
    process::Command,
};

use chrono::{DateTime, Utc};
use db::models::merge::MergeStatus;
use serde::Deserialize;
use tempfile::NamedTempFile;
use thiserror::Error;
use url::Url;
use utils::{command_ext::NoWindowExt, shell::resolve_executable_path_blocking};

use crate::types::{
    CheckSource, CreatePrRequest, MergeMethod, MergeOutcome, PrCheck, PrComment, PrCommentAuthor,
    PrReviewComment, PrState, PullRequestDetail, ReviewCommentUser,
};

#[derive(Debug, Clone)]
pub struct GitHubRepoInfo {
    pub owner: String,
    pub repo_name: String,
    /// GitHub hostname (e.g., "github.com" or enterprise hostname)
    pub hostname: Option<String>,
}

impl GitHubRepoInfo {
    pub fn repo_spec(&self) -> String {
        match &self.hostname {
            Some(host) => format!("{}/{}/{}", host, self.owner, self.repo_name),
            None => format!("{}/{}", self.owner, self.repo_name),
        }
    }
}

#[derive(Deserialize)]
struct GhRepoViewResponse {
    owner: GhRepoOwner,
    name: String,
    url: String,
}

#[derive(Deserialize)]
struct GhRepoOwner {
    login: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhCommentResponse {
    id: String,
    author: Option<GhUserLogin>,
    #[serde(default)]
    author_association: String,
    #[serde(default)]
    body: String,
    created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    url: String,
}

#[derive(Deserialize)]
struct GhCommentsWrapper {
    comments: Vec<GhCommentResponse>,
}

#[derive(Deserialize)]
struct GhUserLogin {
    login: Option<String>,
}

#[derive(Deserialize)]
struct GhReviewCommentResponse {
    id: i64,
    user: Option<GhUserLogin>,
    #[serde(default)]
    body: String,
    created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    path: String,
    line: Option<i64>,
    side: Option<String>,
    #[serde(default)]
    diff_hunk: String,
    #[serde(default)]
    author_association: String,
}

#[derive(Deserialize)]
struct GhMergeCommit {
    oid: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPrResponse {
    number: i64,
    url: String,
    #[serde(default)]
    state: String,
    merged_at: Option<DateTime<Utc>>,
    merge_commit: Option<GhMergeCommit>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    base_ref_name: Option<String>,
    #[serde(default)]
    head_ref_name: Option<String>,
    #[serde(default)]
    updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Error)]
pub enum GhCliError {
    #[error("GitHub CLI (`gh`) executable not found or not runnable")]
    NotAvailable,
    #[error("GitHub CLI command failed: {0}")]
    CommandFailed(String),
    #[error("GitHub CLI authentication failed: {0}")]
    AuthFailed(String),
    #[error("GitHub CLI returned unexpected output: {0}")]
    UnexpectedOutput(String),
}

#[derive(Debug, Clone, Default)]
pub struct GhCli;

impl GhCli {
    pub fn new() -> Self {
        Self {}
    }

    /// Ensure the GitHub CLI binary is discoverable.
    fn ensure_available(&self) -> Result<(), GhCliError> {
        resolve_executable_path_blocking("gh").ok_or(GhCliError::NotAvailable)?;
        Ok(())
    }

    fn run<I, S>(&self, args: I, dir: Option<&Path>) -> Result<String, GhCliError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.ensure_available()?;
        let gh = resolve_executable_path_blocking("gh").ok_or(GhCliError::NotAvailable)?;
        let mut cmd = Command::new(&gh);
        if let Some(d) = dir {
            cmd.current_dir(d);
        }
        for arg in args {
            cmd.arg(arg);
        }
        let output = cmd
            .no_window()
            .output()
            .map_err(|err| GhCliError::CommandFailed(err.to_string()))?;

        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        // Check exit code first - gh CLI uses exit code 4 for auth failures
        if output.status.code() == Some(4) {
            return Err(GhCliError::AuthFailed(stderr));
        }

        // Fall back to string matching for older gh versions or other auth scenarios
        let lower = stderr.to_ascii_lowercase();
        if lower.contains("authentication failed")
            || lower.contains("must authenticate")
            || lower.contains("bad credentials")
            || lower.contains("unauthorized")
            || lower.contains("gh auth login")
        {
            return Err(GhCliError::AuthFailed(stderr));
        }

        Err(GhCliError::CommandFailed(stderr))
    }

    pub fn get_repo_info(
        &self,
        remote_url: &str,
        repo_path: &Path,
    ) -> Result<GitHubRepoInfo, GhCliError> {
        let raw = self.run(
            ["repo", "view", remote_url, "--json", "owner,name,url"],
            Some(repo_path),
        )?;
        Self::parse_repo_info_response(&raw)
    }

    fn parse_repo_info_response(raw: &str) -> Result<GitHubRepoInfo, GhCliError> {
        let resp: GhRepoViewResponse = serde_json::from_str(raw).map_err(|e| {
            GhCliError::UnexpectedOutput(format!("Failed to parse gh repo view response: {e}"))
        })?;

        let hostname = Url::parse(&resp.url)
            .ok()
            .and_then(|u| u.host_str().map(String::from));

        Ok(GitHubRepoInfo {
            owner: resp.owner.login,
            repo_name: resp.name,
            hostname,
        })
    }

    /// Run `gh pr create` and parse the response.
    ///
    /// The `repo_path` parameter specifies the working directory for the command.
    /// This is required for compatibility with older `gh` CLI versions (e.g., v2.4.0)
    /// that require running from within a git repository.
    pub fn create_pr(
        &self,
        request: &CreatePrRequest,
        repo_info: &GitHubRepoInfo,
        repo_path: &Path,
    ) -> Result<PullRequestDetail, GhCliError> {
        // Write body to temp file to avoid shell escaping and length issues
        let body = request.body.as_deref().unwrap_or("");
        let mut body_file = NamedTempFile::new()
            .map_err(|e| GhCliError::CommandFailed(format!("Failed to create temp file: {e}")))?;
        body_file
            .write_all(body.as_bytes())
            .map_err(|e| GhCliError::CommandFailed(format!("Failed to write body: {e}")))?;

        let repo_spec = repo_info.repo_spec();

        let mut args: Vec<OsString> = Vec::with_capacity(14);
        args.push(OsString::from("pr"));
        args.push(OsString::from("create"));
        args.push(OsString::from("--repo"));
        args.push(OsString::from(&repo_spec));
        args.push(OsString::from("--head"));
        args.push(OsString::from(&request.head_branch));
        args.push(OsString::from("--base"));
        args.push(OsString::from(&request.base_branch));
        args.push(OsString::from("--title"));
        args.push(OsString::from(&request.title));
        args.push(OsString::from("--body-file"));
        args.push(body_file.path().as_os_str().to_os_string());

        if request.draft.unwrap_or(false) {
            args.push(OsString::from("--draft"));
        }

        let raw = self.run(args, Some(repo_path))?;
        Self::parse_pr_create_text(&raw, request)
    }

    /// Retrieve details for a pull request by URL.
    pub fn view_pr(&self, pr_url: &str) -> Result<PullRequestDetail, GhCliError> {
        let raw = self.run(
            [
                "pr",
                "view",
                pr_url,
                "--json",
                "number,url,state,mergedAt,mergeCommit,title,baseRefName,headRefName",
            ],
            None,
        )?;
        Self::parse_pr_view(&raw)
    }

    /// List pull requests for a branch (includes closed/merged).
    pub fn list_prs_for_branch(
        &self,
        repo_info: &GitHubRepoInfo,
        branch: &str,
    ) -> Result<Vec<PullRequestDetail>, GhCliError> {
        let repo_spec = repo_info.repo_spec();
        let raw = self.run(
            [
                "pr",
                "list",
                "--repo",
                &repo_spec,
                "--state",
                "all",
                "--head",
                branch,
                "--json",
                "number,url,title,headRefName,baseRefName,state,mergedAt,mergeCommit",
            ],
            None,
        )?;
        Self::parse_pr_list(&raw)
    }

    pub fn list_prs(&self, owner: &str, repo: &str) -> Result<Vec<PullRequestDetail>, GhCliError> {
        let repo_spec = format!("{owner}/{repo}");
        let json_fields =
            "number,url,title,headRefName,baseRefName,state,mergedAt,mergeCommit,updatedAt";

        let open_raw = self.run(
            [
                "pr",
                "list",
                "--repo",
                &repo_spec,
                "--state",
                "open",
                "--json",
                json_fields,
            ],
            None,
        )?;

        let closed_raw = self.run(
            [
                "pr",
                "list",
                "--repo",
                &repo_spec,
                "--state",
                "closed",
                "--limit",
                "20",
                "--json",
                json_fields,
            ],
            None,
        )?;

        let mut open_prs: Vec<GhPrResponse> =
            serde_json::from_str(open_raw.trim()).map_err(|err| {
                GhCliError::UnexpectedOutput(format!(
                    "Failed to parse gh pr list (open) response: {err}; raw: {open_raw}"
                ))
            })?;
        let closed_prs: Vec<GhPrResponse> =
            serde_json::from_str(closed_raw.trim()).map_err(|err| {
                GhCliError::UnexpectedOutput(format!(
                    "Failed to parse gh pr list (closed) response: {err}; raw: {closed_raw}"
                ))
            })?;

        open_prs.extend(closed_prs);
        open_prs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(open_prs
            .into_iter()
            .map(Self::pr_response_to_detail)
            .collect())
    }

    /// Fetch comments for a pull request.
    pub fn get_pr_comments(
        &self,
        repo_info: &GitHubRepoInfo,
        pr_number: i64,
    ) -> Result<Vec<PrComment>, GhCliError> {
        let repo_spec = repo_info.repo_spec();
        let raw = self.run(
            [
                "pr",
                "view",
                &pr_number.to_string(),
                "--repo",
                &repo_spec,
                "--json",
                "comments",
            ],
            None,
        )?;
        Self::parse_pr_comments(&raw)
    }

    /// Fetch inline review comments for a pull request via API.
    pub fn get_pr_review_comments(
        &self,
        repo_info: &GitHubRepoInfo,
        pr_number: i64,
    ) -> Result<Vec<PrReviewComment>, GhCliError> {
        let mut args = vec![
            "api".to_string(),
            format!(
                "repos/{}/{}/pulls/{}/comments",
                repo_info.owner, repo_info.repo_name, pr_number
            ),
        ];
        if let Some(ref host) = repo_info.hostname {
            args.push("--hostname".to_string());
            args.push(host.clone());
        }
        let raw = self.run(args, None)?;
        Self::parse_pr_review_comments(&raw)
    }

    pub fn pr_checkout(
        &self,
        repo_path: &Path,
        owner: &str,
        repo: &str,
        pr_number: i64,
    ) -> Result<(), GhCliError> {
        self.run(
            [
                "pr",
                "checkout",
                &pr_number.to_string(),
                "--repo",
                &format!("{owner}/{repo}"),
                "--force",
            ],
            Some(repo_path),
        )?;
        Ok(())
    }
}

impl GhCli {
    fn parse_pr_create_text(
        raw: &str,
        request: &CreatePrRequest,
    ) -> Result<PullRequestDetail, GhCliError> {
        let pr_url = raw
            .lines()
            .rev()
            .flat_map(|line| line.split_whitespace())
            .map(|token| token.trim_matches(|c: char| c == '<' || c == '>'))
            .find(|token| token.starts_with("http") && token.contains("/pull/"))
            .ok_or_else(|| {
                GhCliError::UnexpectedOutput(format!(
                    "gh pr create did not return a pull request URL; raw output: {raw}"
                ))
            })?
            .trim_end_matches(['.', ',', ';'])
            .to_string();

        let number = pr_url
            .rsplit('/')
            .next()
            .ok_or_else(|| {
                GhCliError::UnexpectedOutput(format!(
                    "Failed to extract PR number from URL '{pr_url}'"
                ))
            })?
            .trim_end_matches(|c: char| !c.is_ascii_digit())
            .parse::<i64>()
            .map_err(|err| {
                GhCliError::UnexpectedOutput(format!(
                    "Failed to parse PR number from URL '{pr_url}': {err}"
                ))
            })?;

        Ok(PullRequestDetail {
            number,
            url: pr_url,
            status: MergeStatus::Open,
            merged_at: None,
            merge_commit_sha: None,
            title: request.title.clone(),
            base_branch: request.base_branch.clone(),
            head_branch: request.head_branch.clone(),
        })
    }

    fn parse_pr_view(raw: &str) -> Result<PullRequestDetail, GhCliError> {
        let pr: GhPrResponse = serde_json::from_str(raw.trim()).map_err(|err| {
            GhCliError::UnexpectedOutput(format!(
                "Failed to parse gh pr view response: {err}; raw: {raw}"
            ))
        })?;
        Ok(Self::pr_response_to_detail(pr))
    }

    fn parse_pr_list(raw: &str) -> Result<Vec<PullRequestDetail>, GhCliError> {
        let prs: Vec<GhPrResponse> = serde_json::from_str(raw.trim()).map_err(|err| {
            GhCliError::UnexpectedOutput(format!(
                "Failed to parse gh pr list response: {err}; raw: {raw}"
            ))
        })?;
        Ok(prs.into_iter().map(Self::pr_response_to_detail).collect())
    }

    fn pr_response_to_detail(pr: GhPrResponse) -> PullRequestDetail {
        let state = if pr.state.is_empty() {
            "OPEN"
        } else {
            &pr.state
        };
        PullRequestDetail {
            number: pr.number,
            url: pr.url,
            status: match state.to_ascii_uppercase().as_str() {
                "OPEN" => MergeStatus::Open,
                "MERGED" => MergeStatus::Merged,
                "CLOSED" => MergeStatus::Closed,
                _ => MergeStatus::Unknown,
            },
            merged_at: pr.merged_at,
            merge_commit_sha: pr.merge_commit.and_then(|c| c.oid),
            title: pr.title.unwrap_or_default(),
            base_branch: pr.base_ref_name.unwrap_or_default(),
            head_branch: pr.head_ref_name.unwrap_or_default(),
        }
    }

    fn parse_pr_comments(raw: &str) -> Result<Vec<PrComment>, GhCliError> {
        let wrapper: GhCommentsWrapper = serde_json::from_str(raw.trim()).map_err(|err| {
            GhCliError::UnexpectedOutput(format!(
                "Failed to parse gh pr view --json comments response: {err}; raw: {raw}"
            ))
        })?;

        Ok(wrapper
            .comments
            .into_iter()
            .map(|c| PrComment {
                id: c.id,
                author: PrCommentAuthor {
                    login: c
                        .author
                        .and_then(|a| a.login)
                        .unwrap_or_else(|| "unknown".to_string()),
                },
                author_association: c.author_association,
                body: c.body,
                created_at: c.created_at.unwrap_or_else(Utc::now),
                url: c.url,
            })
            .collect())
    }

    fn parse_pr_review_comments(raw: &str) -> Result<Vec<PrReviewComment>, GhCliError> {
        let items: Vec<GhReviewCommentResponse> =
            serde_json::from_str(raw.trim()).map_err(|err| {
                GhCliError::UnexpectedOutput(format!(
                    "Failed to parse review comments API response: {err}; raw: {raw}"
                ))
            })?;

        Ok(items
            .into_iter()
            .map(|c| PrReviewComment {
                id: c.id,
                user: ReviewCommentUser {
                    login: c
                        .user
                        .and_then(|u| u.login)
                        .unwrap_or_else(|| "unknown".to_string()),
                },
                body: c.body,
                created_at: c.created_at.unwrap_or_else(Utc::now),
                html_url: c.html_url,
                path: c.path,
                line: c.line,
                side: c.side,
                diff_hunk: c.diff_hunk,
                author_association: c.author_association,
            })
            .collect())
    }
}

// ---------------------------------------------------------------------------
// PR management over the REST API (`gh api`).
//
// Every call runs with `dir = repo_path` so a deployment's owner-routed `gh`
// wrapper can pick the org credential from the checkout's remote (`gh api` has
// no `--repo` flag). `statusCheckRollup` is deliberately never requested: a
// token without the Checks permission fails the whole GraphQL query.
// ---------------------------------------------------------------------------

/// Most runs read for one head commit before the list counts as truncated.
const MAX_ACTIONS_RUNS: usize = 50;
/// GitHub's page maximum, used for check runs, statuses and jobs per run.
const PAGE_SIZE: usize = 100;

#[derive(Deserialize)]
struct RestRef {
    #[serde(default)]
    sha: String,
    #[serde(rename = "ref", default)]
    ref_name: String,
    repo: Option<RestRepoName>,
}

#[derive(Deserialize)]
struct RestRepoName {
    full_name: String,
}

#[derive(Deserialize)]
struct RestPullRequest {
    number: i64,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    merged: bool,
    mergeable: Option<bool>,
    mergeable_state: Option<String>,
    head: RestRef,
    base: RestRef,
    merged_at: Option<DateTime<Utc>>,
    merge_commit_sha: Option<String>,
}

#[derive(Deserialize)]
struct RestCheckRuns {
    #[serde(default)]
    total_count: usize,
    #[serde(default)]
    check_runs: Vec<RestCheckRun>,
}

#[derive(Deserialize)]
struct RestCheckRun {
    #[serde(default)]
    name: String,
    #[serde(default)]
    status: String,
    conclusion: Option<String>,
    details_url: Option<String>,
    html_url: Option<String>,
}

#[derive(Deserialize)]
struct RestCombinedStatus {
    #[serde(default)]
    total_count: usize,
    #[serde(default)]
    statuses: Vec<RestStatus>,
}

#[derive(Deserialize)]
struct RestStatus {
    #[serde(default)]
    context: String,
    #[serde(default)]
    state: String,
    target_url: Option<String>,
}

#[derive(Deserialize)]
struct RestWorkflowRuns {
    #[serde(default)]
    total_count: usize,
    #[serde(default)]
    workflow_runs: Vec<RestWorkflowRun>,
}

#[derive(Deserialize)]
struct RestWorkflowRun {
    id: i64,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    status: Option<String>,
    conclusion: Option<String>,
    html_url: Option<String>,
}

#[derive(Deserialize)]
struct RestJobs {
    #[serde(default)]
    total_count: usize,
    #[serde(default)]
    jobs: Vec<RestJob>,
}

#[derive(Deserialize)]
struct RestJob {
    #[serde(default)]
    name: String,
    #[serde(default)]
    status: String,
    conclusion: Option<String>,
    html_url: Option<String>,
}

#[derive(Deserialize)]
struct RestMergeResponse {
    sha: Option<String>,
    #[serde(default)]
    merged: bool,
    #[serde(default)]
    message: String,
}

/// A page of checks plus whether the provider reported more than it returned.
pub type CheckPage = (Vec<PrCheck>, bool);

fn lower(value: &str) -> String {
    value.to_ascii_lowercase()
}

fn parse_json<'a, T: Deserialize<'a>>(raw: &'a str, what: &str) -> Result<T, GhCliError> {
    serde_json::from_str(raw.trim())
        .map_err(|err| GhCliError::UnexpectedOutput(format!("Failed to parse {what}: {err}")))
}

impl GhCli {
    fn api_args(repo_info: &GitHubRepoInfo, method: Option<&str>, path: String) -> Vec<OsString> {
        let mut args = vec![OsString::from("api")];
        if let Some(method) = method {
            args.push(OsString::from("--method"));
            args.push(OsString::from(method));
        }
        args.push(OsString::from(format!(
            "repos/{}/{}/{}",
            repo_info.owner, repo_info.repo_name, path
        )));
        if let Some(host) = &repo_info.hostname {
            args.push(OsString::from("--hostname"));
            args.push(OsString::from(host));
        }
        args
    }

    fn api_with_body(
        &self,
        repo_info: &GitHubRepoInfo,
        method: &str,
        path: String,
        body: &serde_json::Value,
        repo_path: &Path,
    ) -> Result<String, GhCliError> {
        // A body file keeps PR text out of argv and away from length limits.
        let mut file = NamedTempFile::new()
            .map_err(|e| GhCliError::CommandFailed(format!("Failed to create temp file: {e}")))?;
        file.write_all(body.to_string().as_bytes())
            .map_err(|e| GhCliError::CommandFailed(format!("Failed to write request body: {e}")))?;
        let mut args = Self::api_args(repo_info, Some(method), path);
        args.push(OsString::from("--input"));
        args.push(file.path().as_os_str().to_os_string());
        self.run(args, Some(repo_path))
    }

    pub fn get_pr_state(
        &self,
        repo_info: &GitHubRepoInfo,
        number: i64,
        repo_path: &Path,
    ) -> Result<PrState, GhCliError> {
        let raw = self.run(
            Self::api_args(repo_info, None, format!("pulls/{number}")),
            Some(repo_path),
        )?;
        Self::parse_pr_state(&raw)
    }

    pub fn list_check_runs(
        &self,
        repo_info: &GitHubRepoInfo,
        sha: &str,
        repo_path: &Path,
    ) -> Result<CheckPage, GhCliError> {
        let raw = self.run(
            Self::api_args(
                repo_info,
                None,
                format!("commits/{sha}/check-runs?per_page={PAGE_SIZE}"),
            ),
            Some(repo_path),
        )?;
        Self::parse_check_runs(&raw)
    }

    pub fn get_combined_status(
        &self,
        repo_info: &GitHubRepoInfo,
        sha: &str,
        repo_path: &Path,
    ) -> Result<CheckPage, GhCliError> {
        let raw = self.run(
            Self::api_args(
                repo_info,
                None,
                format!("commits/{sha}/status?per_page={PAGE_SIZE}"),
            ),
            Some(repo_path),
        )?;
        Self::parse_combined_status(&raw)
    }

    /// Workflow jobs for `sha`: the substitute for check runs when the token
    /// lacks the Checks permission (Actions: read is enough).
    pub fn list_actions_jobs(
        &self,
        repo_info: &GitHubRepoInfo,
        sha: &str,
        repo_path: &Path,
    ) -> Result<CheckPage, GhCliError> {
        let raw = self.run(
            Self::api_args(
                repo_info,
                None,
                format!("actions/runs?head_sha={sha}&per_page={MAX_ACTIONS_RUNS}"),
            ),
            Some(repo_path),
        )?;
        let runs: RestWorkflowRuns = parse_json(&raw, "workflow runs")?;
        let mut truncated = runs.total_count > runs.workflow_runs.len();
        let mut checks = Vec::new();
        for run in runs.workflow_runs {
            let raw_jobs = self.run(
                Self::api_args(
                    repo_info,
                    None,
                    format!("actions/runs/{}/jobs?per_page={PAGE_SIZE}", run.id),
                ),
                Some(repo_path),
            )?;
            let (jobs, jobs_truncated) = Self::parse_run_jobs(&run, &raw_jobs)?;
            truncated |= jobs_truncated;
            checks.extend(jobs);
        }
        Ok((checks, truncated))
    }

    /// Merge with a head-SHA guard: GitHub rejects the merge (409) if the
    /// branch moved after `sha` was evaluated.
    pub fn merge_pr(
        &self,
        repo_info: &GitHubRepoInfo,
        number: i64,
        method: MergeMethod,
        sha: &str,
        repo_path: &Path,
    ) -> Result<MergeOutcome, GhCliError> {
        let raw = self.api_with_body(
            repo_info,
            "PUT",
            format!("pulls/{number}/merge"),
            &serde_json::json!({ "merge_method": method.as_str(), "sha": sha }),
            repo_path,
        )?;
        Self::parse_merge_response(&raw)
    }

    /// Delete the remote branch only. Never touches a local checkout, unlike
    /// `gh pr merge --delete-branch`.
    pub fn delete_remote_branch(
        &self,
        repo_info: &GitHubRepoInfo,
        branch: &str,
        repo_path: &Path,
    ) -> Result<(), GhCliError> {
        self.run(
            Self::api_args(
                repo_info,
                Some("DELETE"),
                format!("git/refs/heads/{branch}"),
            ),
            Some(repo_path),
        )?;
        Ok(())
    }

    pub fn patch_pr(
        &self,
        repo_info: &GitHubRepoInfo,
        number: i64,
        title: Option<&str>,
        body: Option<&str>,
        repo_path: &Path,
    ) -> Result<(), GhCliError> {
        let mut fields = serde_json::Map::new();
        if let Some(title) = title {
            fields.insert("title".into(), title.into());
        }
        if let Some(body) = body {
            fields.insert("body".into(), body.into());
        }
        self.api_with_body(
            repo_info,
            "PATCH",
            format!("pulls/{number}"),
            &serde_json::Value::Object(fields),
            repo_path,
        )?;
        Ok(())
    }

    /// Draft ⇄ ready has no REST endpoint; `gh pr ready` drives the GraphQL
    /// mutation. `--repo` lets the owner router pick the credential.
    pub fn set_pr_ready(
        &self,
        repo_info: &GitHubRepoInfo,
        number: i64,
        ready: bool,
        repo_path: &Path,
    ) -> Result<(), GhCliError> {
        let number = number.to_string();
        let repo_spec = repo_info.repo_spec();
        let mut args = vec!["pr", "ready", number.as_str(), "--repo", repo_spec.as_str()];
        if !ready {
            args.push("--undo");
        }
        self.run(args, Some(repo_path))?;
        Ok(())
    }

    fn parse_pr_state(raw: &str) -> Result<PrState, GhCliError> {
        let pr: RestPullRequest = parse_json(raw, "pull request")?;
        let state = if pr.merged {
            MergeStatus::Merged
        } else {
            match lower(&pr.state).as_str() {
                "open" => MergeStatus::Open,
                "closed" => MergeStatus::Closed,
                _ => MergeStatus::Unknown,
            }
        };
        Ok(PrState {
            number: pr.number,
            url: pr.html_url,
            title: pr.title,
            state,
            draft: pr.draft,
            merged: pr.merged,
            mergeable: pr.mergeable,
            merge_state: pr
                .mergeable_state
                .map(|state| lower(&state))
                .unwrap_or_else(|| "unknown".to_string()),
            head_sha: pr.head.sha,
            head_branch: pr.head.ref_name,
            base_branch: pr.base.ref_name,
            head_repo: pr.head.repo.map(|repo| repo.full_name),
            base_repo: pr.base.repo.map(|repo| repo.full_name),
            merged_at: pr.merged_at,
            // GitHub fills this with a test-merge commit for open PRs.
            merge_commit_sha: if pr.merged { pr.merge_commit_sha } else { None },
        })
    }

    fn parse_check_runs(raw: &str) -> Result<CheckPage, GhCliError> {
        let page: RestCheckRuns = parse_json(raw, "check runs")?;
        let truncated = page.total_count > page.check_runs.len();
        let checks = page
            .check_runs
            .into_iter()
            .map(|run| PrCheck {
                name: run.name,
                source: CheckSource::CheckRuns,
                status: lower(&run.status),
                conclusion: run.conclusion.map(|c| lower(&c)),
                url: run.details_url.or(run.html_url),
            })
            .collect();
        Ok((checks, truncated))
    }

    fn parse_combined_status(raw: &str) -> Result<CheckPage, GhCliError> {
        let page: RestCombinedStatus = parse_json(raw, "commit status")?;
        let truncated = page.total_count > page.statuses.len();
        let checks = page
            .statuses
            .into_iter()
            .map(|status| {
                let state = lower(&status.state);
                PrCheck {
                    name: status.context,
                    source: CheckSource::CommitStatuses,
                    conclusion: (state != "pending").then(|| state.clone()),
                    status: state,
                    url: status.target_url,
                }
            })
            .collect();
        Ok((checks, truncated))
    }

    fn parse_run_jobs(run: &RestWorkflowRun, raw: &str) -> Result<CheckPage, GhCliError> {
        let page: RestJobs = parse_json(raw, "workflow jobs")?;
        let run_name = run
            .name
            .clone()
            .unwrap_or_else(|| format!("run {}", run.id));
        if page.jobs.is_empty() {
            // Jobs are not materialised yet: the run itself stands in, so a
            // queued workflow still reads as pending rather than absent.
            return Ok((
                vec![PrCheck {
                    name: run_name,
                    source: CheckSource::ActionsJobs,
                    status: run
                        .status
                        .as_deref()
                        .map(lower)
                        .unwrap_or_else(|| "queued".to_string()),
                    conclusion: run.conclusion.as_deref().map(lower),
                    url: run.html_url.clone(),
                }],
                false,
            ));
        }
        let truncated = page.total_count > page.jobs.len();
        let checks = page
            .jobs
            .into_iter()
            .map(|job| PrCheck {
                name: format!("{run_name} / {}", job.name),
                source: CheckSource::ActionsJobs,
                status: lower(&job.status),
                conclusion: job.conclusion.map(|c| lower(&c)),
                url: job.html_url,
            })
            .collect();
        Ok((checks, truncated))
    }

    fn parse_merge_response(raw: &str) -> Result<MergeOutcome, GhCliError> {
        let response: RestMergeResponse = parse_json(raw, "merge response")?;
        Ok(MergeOutcome {
            merged: response.merged,
            sha: response.sha,
            message: response.message,
            branch_deleted: None,
            branch_delete_error: None,
        })
    }
}

#[cfg(test)]
mod pr_management_parser_tests {
    use super::*;

    #[test]
    fn pr_state_reads_rest_fields_and_hides_test_merge_sha() {
        let raw = r#"{"number":1387,"html_url":"https://github.com/o/r/pull/1387","title":"t",
            "state":"open","draft":false,"merged":false,"mergeable":true,"mergeable_state":"blocked",
            "head":{"sha":"1b1e","ref":"vk/x","repo":{"full_name":"o/r"}},
            "base":{"sha":"aaaa","ref":"main","repo":{"full_name":"o/r"}},
            "merged_at":null,"merge_commit_sha":"testmerge"}"#;
        let pr = GhCli::parse_pr_state(raw).unwrap();
        assert!(matches!(pr.state, MergeStatus::Open));
        assert_eq!(pr.merge_state, "blocked");
        assert_eq!(pr.head_sha, "1b1e");
        assert_eq!(pr.head_branch, "vk/x");
        assert_eq!(pr.base_branch, "main");
        assert_eq!(pr.mergeable, Some(true));
        assert_eq!(pr.merge_commit_sha, None);
        assert!(pr.head_in_base_repo());

        let merged = r#"{"number":1,"state":"closed","merged":true,"mergeable":null,
            "mergeable_state":"unknown","head":{"sha":"h","ref":"b"},"base":{"ref":"main"},
            "merged_at":"2026-09-19T18:53:43Z","merge_commit_sha":"9a96"}"#;
        let pr = GhCli::parse_pr_state(merged).unwrap();
        assert!(matches!(pr.state, MergeStatus::Merged));
        assert_eq!(pr.merge_commit_sha.as_deref(), Some("9a96"));
        assert_eq!(pr.mergeable, None);
    }

    #[test]
    fn check_runs_and_statuses_report_truncation() {
        let (runs, truncated) = GhCli::parse_check_runs(
            r#"{"total_count":2,"check_runs":[{"name":"test","status":"COMPLETED",
            "conclusion":"SUCCESS","details_url":"https://ci/1"}]}"#,
        )
        .unwrap();
        assert!(truncated);
        assert_eq!(runs[0].status, "completed");
        assert_eq!(runs[0].conclusion.as_deref(), Some("success"));
        assert_eq!(runs[0].url.as_deref(), Some("https://ci/1"));

        let (statuses, truncated) = GhCli::parse_combined_status(
            r#"{"state":"pending","total_count":2,"statuses":[
            {"context":"ci/a","state":"pending","target_url":null},
            {"context":"ci/b","state":"failure","target_url":"https://ci/b"}]}"#,
        )
        .unwrap();
        assert!(!truncated);
        assert_eq!(statuses[0].conclusion, None);
        assert!(!statuses[0].is_finished());
        assert!(statuses[1].is_failed());
    }

    #[test]
    fn jobs_are_named_by_workflow_and_empty_runs_stand_in_as_pending() {
        let run = RestWorkflowRun {
            id: 9,
            name: Some("deploy-invariants".into()),
            status: Some("queued".into()),
            conclusion: None,
            html_url: Some("https://github.com/o/r/actions/runs/9".into()),
        };
        let (jobs, truncated) = GhCli::parse_run_jobs(
            &run,
            r#"{"total_count":1,"jobs":[{"name":"check","status":"in_progress","conclusion":null,
            "html_url":"https://github.com/o/r/actions/runs/9/job/1"}]}"#,
        )
        .unwrap();
        assert!(!truncated);
        assert_eq!(jobs[0].name, "deploy-invariants / check");
        assert_eq!(jobs[0].source, CheckSource::ActionsJobs);

        let (placeholder, _) =
            GhCli::parse_run_jobs(&run, r#"{"total_count":0,"jobs":[]}"#).unwrap();
        assert_eq!(placeholder.len(), 1);
        assert_eq!(placeholder[0].name, "deploy-invariants");
        assert!(!placeholder[0].is_finished());
    }

    #[test]
    fn merge_response_parses() {
        let outcome = GhCli::parse_merge_response(
            r#"{"sha":"abc","merged":true,"message":"Pull Request successfully merged"}"#,
        )
        .unwrap();
        assert!(outcome.merged);
        assert_eq!(outcome.sha.as_deref(), Some("abc"));
        assert_eq!(outcome.branch_deleted, None);
    }

    #[test]
    fn api_args_scope_to_repo_and_host() {
        let info = GitHubRepoInfo {
            owner: "o".into(),
            repo_name: "r".into(),
            hostname: Some("ghe.example".into()),
        };
        let args: Vec<String> = GhCli::api_args(&info, Some("PUT"), "pulls/3/merge".into())
            .into_iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "api",
                "--method",
                "PUT",
                "repos/o/r/pulls/3/merge",
                "--hostname",
                "ghe.example"
            ]
        );
    }
}
