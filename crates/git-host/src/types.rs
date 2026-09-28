use chrono::{DateTime, Utc};
use db::models::merge::{MergeStatus, PullRequestInfo};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    GitHub,
    AzureDevOps,
    Unknown,
}

impl std::fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderKind::GitHub => write!(f, "GitHub"),
            ProviderKind::AzureDevOps => write!(f, "Azure DevOps"),
            ProviderKind::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CreatePrRequest {
    pub title: String,
    pub body: Option<String>,
    pub head_branch: String,
    pub base_branch: String,
    pub draft: Option<bool>,
    /// URL of the repo containing the head branch (for cross-fork PRs).
    pub head_repo_url: Option<String>,
}

#[derive(Debug, Error)]
pub enum GitHostError {
    #[error("Repository error: {0}")]
    Repository(String),
    #[error("Pull request error: {0}")]
    PullRequest(String),
    #[error("Authentication failed: {0}")]
    AuthFailed(String),
    #[error("Insufficient permissions: {0}")]
    InsufficientPermissions(String),
    #[error("Repository not found or no access: {0}")]
    RepoNotFoundOrNoAccess(String),
    #[error("{provider} CLI is not installed or not available in PATH")]
    CliNotInstalled { provider: ProviderKind },
    #[error("Not a git repository: {0}")]
    NotAGitRepository(String),
    #[error("Unsupported git hosting provider")]
    UnsupportedProvider,
    #[error("CLI returned unexpected output: {0}")]
    UnexpectedOutput(String),
}

impl GitHostError {
    pub fn should_retry(&self) -> bool {
        !matches!(
            self,
            GitHostError::AuthFailed(_)
                | GitHostError::InsufficientPermissions(_)
                | GitHostError::RepoNotFoundOrNoAccess(_)
                | GitHostError::CliNotInstalled { .. }
                | GitHostError::NotAGitRepository(_)
                | GitHostError::UnsupportedProvider
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PrCommentAuthor {
    pub login: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PrComment {
    pub id: String,
    pub author: PrCommentAuthor,
    pub author_association: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ReviewCommentUser {
    pub login: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PrReviewComment {
    pub id: i64,
    pub user: ReviewCommentUser,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub html_url: String,
    pub path: String,
    pub line: Option<i64>,
    pub side: Option<String>,
    pub diff_hunk: String,
    pub author_association: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "comment_type", rename_all = "snake_case")]
#[ts(tag = "comment_type", rename_all = "snake_case")]
pub enum UnifiedPrComment {
    General {
        id: String,
        author: String,
        author_association: Option<String>,
        body: String,
        created_at: DateTime<Utc>,
        url: Option<String>,
    },
    Review {
        id: i64,
        author: String,
        author_association: Option<String>,
        body: String,
        created_at: DateTime<Utc>,
        url: Option<String>,
        path: String,
        line: Option<i64>,
        side: Option<String>,
        diff_hunk: Option<String>,
    },
}

impl UnifiedPrComment {
    pub fn created_at(&self) -> DateTime<Utc> {
        match self {
            UnifiedPrComment::General { created_at, .. } => *created_at,
            UnifiedPrComment::Review { created_at, .. } => *created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PullRequestDetail {
    pub number: i64,
    pub url: String,
    pub status: MergeStatus,
    pub merged_at: Option<DateTime<Utc>>,
    pub merge_commit_sha: Option<String>,
    pub title: String,
    pub base_branch: String,
    pub head_branch: String,
}

impl From<PullRequestDetail> for PullRequestInfo {
    fn from(d: PullRequestDetail) -> Self {
        PullRequestInfo {
            number: d.number,
            url: d.url,
            status: d.status,
            merged_at: d.merged_at,
            merge_commit_sha: d.merge_commit_sha,
        }
    }
}

// ---------------------------------------------------------------------------
// PR management (state, checks, merge, update) — the model behind the VK MCP
// `get_pr` / `list_pr_checks` / `merge_pr` / `update_pr` tools.
// ---------------------------------------------------------------------------

/// Live state of one pull request, read from the hosting provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrState {
    pub number: i64,
    pub url: String,
    pub title: String,
    pub state: MergeStatus,
    pub draft: bool,
    pub merged: bool,
    /// `None` while the provider is still computing mergeability.
    pub mergeable: Option<bool>,
    /// GitHub `mergeable_state`, lower-cased: `clean`, `has_hooks`,
    /// `unstable`, `blocked`, `behind`, `dirty`, `draft` or `unknown`. It is
    /// the provider's own verdict across every required check and review, and
    /// reading it needs no Checks permission.
    pub merge_state: String,
    /// The commit the checks describe and the merge is guarded on.
    pub head_sha: String,
    pub head_branch: String,
    pub base_branch: String,
    /// `owner/repo` holding the head branch; differs from `base_repo` for a
    /// PR opened from a fork. `None` when the fork was deleted.
    pub head_repo: Option<String>,
    pub base_repo: Option<String>,
    pub merged_at: Option<DateTime<Utc>>,
    pub merge_commit_sha: Option<String>,
}

impl PrState {
    /// Whether the head branch lives in the base repository, i.e. deleting
    /// `head_branch` there deletes *this* PR's branch and not a same-named
    /// branch belonging to someone else.
    pub fn head_in_base_repo(&self) -> bool {
        match (&self.head_repo, &self.base_repo) {
            (Some(head), Some(base)) => head.eq_ignore_ascii_case(base),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckSource {
    CheckRuns,
    CommitStatuses,
    ActionsJobs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceState {
    Ok,
    Forbidden,
    Error,
}

/// How much of one check source could be read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceCoverage {
    pub source: CheckSource,
    pub state: SourceState,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrCheck {
    pub name: String,
    pub source: CheckSource,
    /// Raw status, lower-cased (`queued`, `in_progress`, `completed`, or the
    /// commit-status state for `commit_statuses`).
    pub status: String,
    pub conclusion: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChecksOverall {
    Passing,
    Failing,
    Pending,
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrChecks {
    pub head_sha: String,
    pub overall: ChecksOverall,
    /// True only when check runs *and* commit statuses were both read in
    /// full. Otherwise non-Actions checks may be missing from `checks`.
    pub complete: bool,
    pub checks: Vec<PrCheck>,
    pub sources: Vec<SourceCoverage>,
}

/// What one check source produced. `truncated` means the provider reported
/// more entries than were fetched, so the list is not a complete total.
#[derive(Debug, Clone, PartialEq)]
pub enum SourceRead {
    Ok {
        checks: Vec<PrCheck>,
        truncated: bool,
    },
    Forbidden(String),
    Error(String),
}

impl SourceRead {
    fn coverage(&self, source: CheckSource) -> SourceCoverage {
        let (state, detail) = match self {
            SourceRead::Ok { truncated, .. } => (
                SourceState::Ok,
                truncated.then(|| "truncated: more entries exist than were read".to_string()),
            ),
            SourceRead::Forbidden(detail) => (SourceState::Forbidden, Some(detail.clone())),
            SourceRead::Error(detail) => (SourceState::Error, Some(detail.clone())),
        };
        SourceCoverage {
            source,
            state,
            detail,
        }
    }

    fn fully_read(&self) -> bool {
        matches!(
            self,
            SourceRead::Ok {
                truncated: false,
                ..
            }
        )
    }

    fn into_checks(self) -> Option<Vec<PrCheck>> {
        match self {
            SourceRead::Ok { checks, .. } => Some(checks),
            _ => None,
        }
    }
}

/// Conclusions that count as a pass. Anything else a *finished* check reports
/// — `failure`, `cancelled`, `timed_out`, `stale`, `action_required`, a
/// status `error`, or a conclusion GitHub adds later — is not a pass, so an
/// unfamiliar value can never make CI read as green.
const PASSING_CONCLUSIONS: &[&str] = &["success", "neutral", "skipped"];

impl PrCheck {
    pub fn is_failed(&self) -> bool {
        self.is_finished()
            && !self
                .conclusion
                .as_deref()
                .is_some_and(|conclusion| PASSING_CONCLUSIONS.contains(&conclusion))
    }

    pub fn is_finished(&self) -> bool {
        match self.source {
            CheckSource::CommitStatuses => self.status != "pending",
            CheckSource::CheckRuns | CheckSource::ActionsJobs => self.status == "completed",
        }
    }
}

/// Combine the three sources into one verdict.
///
/// Check runs already include Actions jobs, so Actions jobs are only used as
/// the substitute when check runs could not be read — never added on top.
pub fn aggregate_checks(
    head_sha: &str,
    check_runs: SourceRead,
    statuses: SourceRead,
    actions_jobs: SourceRead,
) -> PrChecks {
    let sources = vec![
        check_runs.coverage(CheckSource::CheckRuns),
        statuses.coverage(CheckSource::CommitStatuses),
        actions_jobs.coverage(CheckSource::ActionsJobs),
    ];
    let complete = check_runs.fully_read() && statuses.fully_read();

    let mut checks = check_runs
        .into_checks()
        .or_else(|| actions_jobs.into_checks())
        .unwrap_or_default();
    checks.extend(statuses.into_checks().unwrap_or_default());

    let overall = if checks.iter().any(PrCheck::is_failed) {
        ChecksOverall::Failing
    } else if checks.iter().any(|check| !check.is_finished()) {
        ChecksOverall::Pending
    } else if checks.is_empty() {
        ChecksOverall::None
    } else {
        ChecksOverall::Passing
    };

    PrChecks {
        head_sha: head_sha.to_string(),
        overall,
        complete,
        checks,
        sources,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeMethod {
    #[default]
    Squash,
    Merge,
    Rebase,
}

impl MergeMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            MergeMethod::Squash => "squash",
            MergeMethod::Merge => "merge",
            MergeMethod::Rebase => "rebase",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MergeOutcome {
    pub merged: bool,
    pub sha: Option<String>,
    pub message: String,
    /// `None` when branch deletion was not requested.
    pub branch_deleted: Option<bool>,
    pub branch_delete_error: Option<String>,
}

/// Fields to change on a pull request. `ready_for_review: Some(false)`
/// converts it back to a draft.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdatePrFields {
    pub title: Option<String>,
    pub body: Option<String>,
    pub ready_for_review: Option<bool>,
}

impl UpdatePrFields {
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.body.is_none() && self.ready_for_review.is_none()
    }
}

/// Why `merge_gate` refused. `retryable` means waiting and polling again can
/// change the answer (checks still running, mergeability still computing).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeRefusal {
    pub reason: String,
    pub retryable: bool,
}

fn refuse(reason: impl Into<String>, retryable: bool) -> Result<(), MergeRefusal> {
    Err(MergeRefusal {
        reason: reason.into(),
        retryable,
    })
}

fn check_names(checks: &PrChecks, predicate: impl Fn(&PrCheck) -> bool) -> String {
    checks
        .checks
        .iter()
        .filter(|check| predicate(check))
        .map(|check| check.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Decide whether VK may merge `pr` now.
///
/// Draft, conflicted and non-open PRs are refused even with `force`: the
/// provider cannot merge them either. `force` otherwise skips VK's own gate
/// and leaves branch protection to the provider. Without `force`, VK needs
/// both its own reading of the checks *and* the provider's `merge_state`
/// verdict to agree, so a partially readable check list is never enough on
/// its own.
pub fn merge_gate(pr: &PrState, checks: &PrChecks, force: bool) -> Result<(), MergeRefusal> {
    if pr.merged || !matches!(pr.state, MergeStatus::Open) {
        return refuse(
            format!(
                "PR #{} is not open (state: {})",
                pr.number,
                format!("{:?}", pr.state).to_lowercase()
            ),
            false,
        );
    }
    if pr.draft {
        return refuse(
            format!(
                "PR #{} is a draft; mark it ready for review first (update_pr ready_for_review=true)",
                pr.number
            ),
            false,
        );
    }
    if pr.merge_state == "dirty" {
        return refuse(
            format!(
                "PR #{} has merge conflicts with {}; resolve them and push first",
                pr.number, pr.base_branch
            ),
            false,
        );
    }
    if force {
        return Ok(());
    }
    if pr.merge_state == "unknown" {
        return refuse(
            "GitHub is still computing mergeability; call get_pr again shortly",
            true,
        );
    }
    match checks.overall {
        ChecksOverall::Failing => {
            return refuse(
                format!(
                    "checks are failing: {}",
                    check_names(checks, PrCheck::is_failed)
                ),
                false,
            );
        }
        ChecksOverall::Pending => {
            return refuse(
                format!(
                    "checks are still running: {}; poll get_pr until they finish",
                    check_names(checks, |check| !check.is_finished())
                ),
                true,
            );
        }
        ChecksOverall::Passing | ChecksOverall::None => {}
    }
    if !matches!(pr.merge_state.as_str(), "clean" | "has_hooks") {
        return refuse(
            format!(
                "GitHub reports merge state '{}' (not clean): required checks, reviews or an \
                 up-to-date branch are still missing",
                pr.merge_state
            ),
            pr.merge_state == "behind",
        );
    }
    Ok(())
}

/// A pull request named by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrReference {
    Number(i64),
    Url {
        host: String,
        owner: String,
        repo: String,
        number: i64,
    },
}

impl PrReference {
    pub fn number(&self) -> i64 {
        match self {
            PrReference::Number(number) | PrReference::Url { number, .. } => *number,
        }
    }
}

/// Parse `123`, `#123` or `https://<host>/<owner>/<repo>/pull/<n>[/...]`.
pub fn parse_pr_reference(input: &str) -> Result<PrReference, GitHostError> {
    let trimmed = input.trim();
    let invalid = || {
        GitHostError::PullRequest(format!(
            "'{trimmed}' is not a PR number or a https://<host>/<owner>/<repo>/pull/<n> URL"
        ))
    };
    let positive = |raw: &str| {
        raw.parse::<i64>()
            .ok()
            .filter(|number| *number > 0)
            .ok_or_else(invalid)
    };
    if let Ok(url) = url::Url::parse(trimmed) {
        if !matches!(url.scheme(), "http" | "https") {
            return Err(invalid());
        }
        let host = url.host_str().ok_or_else(invalid)?.to_string();
        let segments: Vec<&str> = url
            .path_segments()
            .map(|segments| segments.filter(|s| !s.is_empty()).collect())
            .unwrap_or_default();
        return match segments.as_slice() {
            [owner, repo, "pull", number, ..] => Ok(PrReference::Url {
                host,
                owner: owner.to_string(),
                repo: repo.to_string(),
                number: positive(number)?,
            }),
            _ => Err(invalid()),
        };
    }
    Ok(PrReference::Number(positive(
        trimmed.strip_prefix('#').unwrap_or(trimmed),
    )?))
}

/// Owner/name equality as the provider treats it (case-insensitive).
pub fn same_repo(owner: &str, repo: &str, other_owner: &str, other_repo: &str) -> bool {
    owner.eq_ignore_ascii_case(other_owner)
        && repo
            .trim_end_matches(".git")
            .eq_ignore_ascii_case(other_repo.trim_end_matches(".git"))
}

#[cfg(test)]
mod pr_management_tests {
    use super::*;

    fn check(name: &str, source: CheckSource, status: &str, conclusion: Option<&str>) -> PrCheck {
        PrCheck {
            name: name.to_string(),
            source,
            status: status.to_string(),
            conclusion: conclusion.map(str::to_string),
            url: None,
        }
    }

    fn ok(checks: Vec<PrCheck>) -> SourceRead {
        SourceRead::Ok {
            checks,
            truncated: false,
        }
    }

    fn pr(merge_state: &str) -> PrState {
        PrState {
            number: 7,
            url: "https://github.com/o/r/pull/7".into(),
            title: "t".into(),
            state: MergeStatus::Open,
            draft: false,
            merged: false,
            mergeable: Some(true),
            merge_state: merge_state.into(),
            head_sha: "abc".into(),
            head_branch: "vk/x".into(),
            base_branch: "main".into(),
            head_repo: Some("o/r".into()),
            base_repo: Some("O/R".into()),
            merged_at: None,
            merge_commit_sha: None,
        }
    }

    fn checks_with(overall_checks: Vec<PrCheck>) -> PrChecks {
        aggregate_checks(
            "abc",
            ok(overall_checks),
            ok(vec![]),
            SourceRead::Error("unused".into()),
        )
    }

    #[test]
    fn forbidden_check_runs_fall_back_to_actions_jobs_and_are_incomplete() {
        // The workspace PAT's real shape: no Checks or Commit statuses read.
        let result = aggregate_checks(
            "abc",
            SourceRead::Forbidden(
                "HTTP 403: Resource not accessible by personal access token".into(),
            ),
            SourceRead::Forbidden("HTTP 403".into()),
            ok(vec![check(
                "ci / test",
                CheckSource::ActionsJobs,
                "completed",
                Some("success"),
            )]),
        );
        assert_eq!(result.overall, ChecksOverall::Passing);
        assert!(!result.complete);
        assert_eq!(result.checks.len(), 1);
        assert_eq!(result.sources[0].state, SourceState::Forbidden);
        assert_eq!(result.sources[1].state, SourceState::Forbidden);
        assert_eq!(result.sources[2].state, SourceState::Ok);
    }

    #[test]
    fn readable_check_runs_are_not_double_counted_with_actions_jobs() {
        let result = aggregate_checks(
            "abc",
            ok(vec![check(
                "test",
                CheckSource::CheckRuns,
                "completed",
                Some("success"),
            )]),
            ok(vec![check(
                "ci/legacy",
                CheckSource::CommitStatuses,
                "success",
                Some("success"),
            )]),
            ok(vec![check(
                "test",
                CheckSource::ActionsJobs,
                "completed",
                Some("success"),
            )]),
        );
        assert!(result.complete);
        assert_eq!(result.checks.len(), 2);
        assert!(
            result
                .checks
                .iter()
                .all(|check| check.source != CheckSource::ActionsJobs)
        );
    }

    #[test]
    fn truncated_source_is_reported_and_never_complete() {
        let result = aggregate_checks(
            "abc",
            SourceRead::Ok {
                checks: vec![check(
                    "a",
                    CheckSource::CheckRuns,
                    "completed",
                    Some("success"),
                )],
                truncated: true,
            },
            ok(vec![]),
            ok(vec![]),
        );
        assert!(!result.complete);
        assert!(
            result.sources[0]
                .detail
                .as_deref()
                .is_some_and(|detail| detail.starts_with("truncated"))
        );
    }

    #[test]
    fn overall_prefers_failing_then_pending_then_passing() {
        let failing = checks_with(vec![
            check("a", CheckSource::CheckRuns, "in_progress", None),
            check("b", CheckSource::CheckRuns, "completed", Some("timed_out")),
        ]);
        assert_eq!(failing.overall, ChecksOverall::Failing);

        let pending = checks_with(vec![
            check("a", CheckSource::CheckRuns, "queued", None),
            check("b", CheckSource::CheckRuns, "completed", Some("skipped")),
        ]);
        assert_eq!(pending.overall, ChecksOverall::Pending);

        let passing = checks_with(vec![
            check("a", CheckSource::CheckRuns, "completed", Some("neutral")),
            check("b", CheckSource::CheckRuns, "completed", Some("success")),
        ]);
        assert_eq!(passing.overall, ChecksOverall::Passing);

        assert_eq!(checks_with(vec![]).overall, ChecksOverall::None);

        // Aged-out and unfamiliar conclusions are never a pass.
        for conclusion in ["stale", "action_required", "some_future_value"] {
            let result = checks_with(vec![check(
                "a",
                CheckSource::CheckRuns,
                "completed",
                Some(conclusion),
            )]);
            assert_eq!(result.overall, ChecksOverall::Failing, "{conclusion}");
        }
        let no_conclusion =
            checks_with(vec![check("a", CheckSource::CheckRuns, "completed", None)]);
        assert_eq!(no_conclusion.overall, ChecksOverall::Failing);

        let status_error = aggregate_checks(
            "abc",
            ok(vec![]),
            ok(vec![check(
                "ci",
                CheckSource::CommitStatuses,
                "error",
                Some("error"),
            )]),
            ok(vec![]),
        );
        assert_eq!(status_error.overall, ChecksOverall::Failing);
        let status_pending = aggregate_checks(
            "abc",
            ok(vec![]),
            ok(vec![check(
                "ci",
                CheckSource::CommitStatuses,
                "pending",
                None,
            )]),
            ok(vec![]),
        );
        assert_eq!(status_pending.overall, ChecksOverall::Pending);
    }

    #[test]
    fn gate_allows_clean_passing_and_clean_without_ci() {
        let passing = checks_with(vec![check(
            "a",
            CheckSource::CheckRuns,
            "completed",
            Some("success"),
        )]);
        assert_eq!(merge_gate(&pr("clean"), &passing, false), Ok(()));
        assert_eq!(merge_gate(&pr("has_hooks"), &passing, false), Ok(()));
        assert_eq!(
            merge_gate(&pr("clean"), &checks_with(vec![]), false),
            Ok(())
        );
    }

    #[test]
    fn gate_refuses_failing_and_pending_naming_the_checks() {
        let failing = checks_with(vec![check(
            "lint",
            CheckSource::CheckRuns,
            "completed",
            Some("failure"),
        )]);
        let refusal = merge_gate(&pr("unstable"), &failing, false).unwrap_err();
        assert!(refusal.reason.contains("lint"), "{refusal:?}");
        assert!(!refusal.retryable);

        let pending = checks_with(vec![check(
            "test",
            CheckSource::CheckRuns,
            "in_progress",
            None,
        )]);
        let refusal = merge_gate(&pr("blocked"), &pending, false).unwrap_err();
        assert!(refusal.reason.contains("test"), "{refusal:?}");
        assert!(refusal.retryable);
    }

    #[test]
    fn gate_requires_providers_clean_verdict_even_when_visible_checks_pass() {
        // Actions-only view is green, but a non-Actions required check that
        // the PAT cannot see keeps GitHub at `blocked`.
        let partial = aggregate_checks(
            "abc",
            SourceRead::Forbidden("403".into()),
            SourceRead::Forbidden("403".into()),
            ok(vec![check(
                "ci",
                CheckSource::ActionsJobs,
                "completed",
                Some("success"),
            )]),
        );
        let refusal = merge_gate(&pr("blocked"), &partial, false).unwrap_err();
        assert!(refusal.reason.contains("blocked"), "{refusal:?}");
        let unknown = merge_gate(&pr("unknown"), &partial, false).unwrap_err();
        assert!(unknown.retryable);
    }

    #[test]
    fn force_skips_vk_gate_but_never_draft_conflict_or_closed() {
        let failing = checks_with(vec![check(
            "lint",
            CheckSource::CheckRuns,
            "completed",
            Some("failure"),
        )]);
        assert_eq!(merge_gate(&pr("blocked"), &failing, true), Ok(()));

        let mut draft = pr("clean");
        draft.draft = true;
        assert!(merge_gate(&draft, &failing, true).is_err());

        assert!(merge_gate(&pr("dirty"), &failing, true).is_err());

        let mut merged = pr("clean");
        merged.state = MergeStatus::Merged;
        merged.merged = true;
        assert!(merge_gate(&merged, &failing, true).is_err());
    }

    #[test]
    fn head_branch_is_only_deletable_from_the_repo_that_holds_it() {
        assert!(pr("clean").head_in_base_repo());
        let mut fork = pr("clean");
        fork.head_repo = Some("someone/r".into());
        assert!(!fork.head_in_base_repo());
        fork.head_repo = None;
        assert!(!fork.head_in_base_repo());
    }

    #[test]
    fn pr_references_parse_numbers_and_urls() {
        assert_eq!(parse_pr_reference("42").unwrap(), PrReference::Number(42));
        assert_eq!(
            parse_pr_reference(" #42 ").unwrap(),
            PrReference::Number(42)
        );
        assert_eq!(
            parse_pr_reference("https://github.com/davidvasandani/homelab/pull/1267/checks")
                .unwrap(),
            PrReference::Url {
                host: "github.com".into(),
                owner: "davidvasandani".into(),
                repo: "homelab".into(),
                number: 1267,
            }
        );
        for bad in [
            "",
            "0",
            "-3",
            "abc",
            "https://github.com/o/r/issues/4",
            "https://github.com/o/r/pull/x",
            "ftp://github.com/o/r/pull/4",
        ] {
            assert!(parse_pr_reference(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn same_repo_is_case_insensitive_and_ignores_git_suffix() {
        assert!(same_repo(
            "DavidVasandani",
            "Homelab",
            "davidvasandani",
            "homelab.git"
        ));
        assert!(!same_repo(
            "davidvasandani",
            "homelab",
            "someone",
            "homelab"
        ));
        assert!(!same_repo(
            "davidvasandani",
            "homelab",
            "davidvasandani",
            "vibe-kanban"
        ));
    }
}
