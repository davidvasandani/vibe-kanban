//! Pull-request tools: open, read, poll checks, update and merge the
//! workspace's PR through Vibe Kanban, so agents never need `gh` for it.
//!
//! The backend owns repository paths and credentials; these tools only pick
//! the workspace and repo, then call `/api/workspaces/{id}/pull-requests/*`
//! and pass the JSON result through.

use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool,
    tool_router,
};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use super::{McpServer, ToolError};

const WORKSPACE_ID_HELP: &str = "Workspace ID. Optional inside that workspace's orchestrator \
     context; otherwise required (it is the workspace directory name, or find it with \
     list_workspaces).";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CreatePrToolRequest {
    #[schemars(description = WORKSPACE_ID_HELP)]
    workspace_id: Option<Uuid>,
    #[schemars(
        description = "Repository ID within the workspace. Optional when the workspace has exactly one repository."
    )]
    repo_id: Option<Uuid>,
    #[schemars(description = "Pull request title")]
    title: String,
    #[schemars(description = "Pull request body (Markdown)")]
    body: Option<String>,
    #[schemars(
        description = "Base branch to merge into. Defaults to the workspace repo's target branch."
    )]
    base: Option<String>,
    #[schemars(description = "Open as a draft PR")]
    draft: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct PrTargetToolRequest {
    #[schemars(description = WORKSPACE_ID_HELP)]
    workspace_id: Option<Uuid>,
    #[schemars(
        description = "Repository ID within the workspace. Optional when the workspace has exactly one repository."
    )]
    repo_id: Option<Uuid>,
    #[schemars(
        description = "PR number, '#123', or PR URL (must belong to the workspace repo). Omit to use the workspace's own PR."
    )]
    pr: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum MergeMethodParam {
    Squash,
    Merge,
    Rebase,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct MergePrToolRequest {
    #[schemars(description = WORKSPACE_ID_HELP)]
    workspace_id: Option<Uuid>,
    #[schemars(
        description = "Repository ID within the workspace. Optional when the workspace has exactly one repository."
    )]
    repo_id: Option<Uuid>,
    #[schemars(description = "PR number, '#123', or PR URL. Omit to use the workspace's own PR.")]
    pr: Option<String>,
    #[schemars(description = "Merge method: squash (default), merge or rebase")]
    method: Option<MergeMethodParam>,
    #[schemars(description = "Delete the remote head branch after merging (default false)")]
    delete_branch: Option<bool>,
    #[schemars(
        description = "Skip Vibe Kanban's check/merge-state gate (default false). Only on explicit instruction; drafts and conflicted PRs are refused regardless, and GitHub branch protection still applies."
    )]
    force: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct UpdatePrToolRequest {
    #[schemars(description = WORKSPACE_ID_HELP)]
    workspace_id: Option<Uuid>,
    #[schemars(
        description = "Repository ID within the workspace. Optional when the workspace has exactly one repository."
    )]
    repo_id: Option<Uuid>,
    #[schemars(description = "PR number, '#123', or PR URL. Omit to use the workspace's own PR.")]
    pr: Option<String>,
    #[schemars(description = "New title")]
    title: Option<String>,
    #[schemars(description = "New body (Markdown); replaces the existing body")]
    body: Option<String>,
    #[schemars(
        description = "true marks a draft ready for review; false converts the PR back to a draft"
    )]
    ready_for_review: Option<bool>,
}

/// The PR number at the end of a `…/pull/<n>` URL.
fn pr_number_from_url(url: &str) -> Option<i64> {
    let mut segments = url.trim_end_matches('/').rsplit('/');
    let number = segments.next()?.parse::<i64>().ok()?;
    (segments.next()? == "pull").then_some(number)
}

/// Default `repo_id` to the workspace's only repository.
fn select_single_repo(repos: &[Value]) -> Result<Uuid, ToolError> {
    let parsed: Vec<(Uuid, String)> = repos
        .iter()
        .filter_map(|repo| {
            let id = repo.get("id")?.as_str()?.parse().ok()?;
            let name = repo
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_string();
            Some((id, name))
        })
        .collect();
    match parsed.as_slice() {
        [(id, _)] => Ok(*id),
        [] => Err(ToolError::message("This workspace has no repositories")),
        many => Err(ToolError::new(
            "repo_id is required: this workspace has several repositories",
            Some(
                many.iter()
                    .map(|(id, name)| format!("{name} ({id})"))
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        )),
    }
}

impl McpServer {
    /// Resolve and scope-check the workspace, then default the repo.
    async fn resolve_pr_scope(
        &self,
        workspace_id: Option<Uuid>,
        repo_id: Option<Uuid>,
    ) -> Result<(Uuid, Uuid), ToolError> {
        let workspace_id = self.resolve_workspace_id(workspace_id)?;
        self.scope_allows_workspace(workspace_id)?;
        if let Some(repo_id) = repo_id {
            return Ok((workspace_id, repo_id));
        }
        let url = self.url(&format!("/api/workspaces/{workspace_id}/repos"));
        let repos: Vec<Value> = self.send_json(self.client.get(&url)).await?;
        Ok((workspace_id, select_single_repo(&repos)?))
    }

    async fn pr_get(&self, request: PrTargetToolRequest, route: &str) -> Result<Value, ToolError> {
        let (workspace_id, repo_id) = self
            .resolve_pr_scope(request.workspace_id, request.repo_id)
            .await?;
        let url = self.url(&format!(
            "/api/workspaces/{workspace_id}/pull-requests/{route}"
        ));
        let mut query = vec![("repo_id", repo_id.to_string())];
        if let Some(pr) = request.pr {
            query.push(("pr", pr));
        }
        self.send_json(self.client.get(&url).query(&query)).await
    }
}

#[tool_router(router = pull_requests_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "Open a pull request for the workspace branch: pushes the branch if needed, then opens the PR against `base` (default: the workspace repo's target branch). Returns the PR number and URL. If a PR already exists for the branch, the error says so — use get_pr instead of opening another. Use this rather than `gh pr create`."
    )]
    async fn create_pr(
        &self,
        Parameters(request): Parameters<CreatePrToolRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let (workspace_id, repo_id) = match self
            .resolve_pr_scope(request.workspace_id, request.repo_id)
            .await
        {
            Ok(scope) => scope,
            Err(e) => return Ok(Self::tool_error(e)),
        };
        let url = self.url(&format!("/api/workspaces/{workspace_id}/pull-requests"));
        let payload = serde_json::json!({
            "title": request.title,
            "body": request.body,
            "target_branch": request.base,
            "draft": request.draft,
            "repo_id": repo_id,
        });
        let pr_url: String = match self.send_json(self.client.post(&url).json(&payload)).await {
            Ok(pr_url) => pr_url,
            Err(e) => return Ok(Self::tool_error(e)),
        };
        McpServer::success(&serde_json::json!({
            "number": pr_number_from_url(&pr_url),
            "url": pr_url,
            "repo_id": repo_id,
            "base": request.base,
        }))
    }

    #[tool(
        description = "Get a pull request's live state: open/merged/closed, draft, mergeable, GitHub merge_state (clean/blocked/unstable/behind/dirty/unknown), head SHA and branches, plus a checks summary (see list_pr_checks). Poll this to wait for CI before merge_pr."
    )]
    async fn get_pr(
        &self,
        Parameters(request): Parameters<PrTargetToolRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.pr_get(request, "status").await {
            Ok(value) => McpServer::success(&value),
            Err(e) => Ok(Self::tool_error(e)),
        }
    }

    #[tool(
        description = "List CI checks for the PR head commit: name, source, status, conclusion and a link for each, an overall verdict (passing/failing/pending/none), and per-source coverage. Works without the token's Checks permission by falling back to GitHub Actions jobs; then `complete` is false and non-Actions checks may be missing — never treat an incomplete list as all-green on its own."
    )]
    async fn list_pr_checks(
        &self,
        Parameters(request): Parameters<PrTargetToolRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.pr_get(request, "checks").await {
            Ok(value) => McpServer::success(&value),
            Err(e) => Ok(Self::tool_error(e)),
        }
    }

    #[tool(
        description = "Merge a pull request once CI passes. Refuses unless checks are passing (or there are none) and GitHub's merge_state is clean; pending checks and an 'unknown' merge state are reported as retryable, so poll get_pr and try again. Drafts and conflicted PRs are always refused. Merges exactly the head commit that was checked. Use this rather than `gh pr merge`."
    )]
    async fn merge_pr(
        &self,
        Parameters(request): Parameters<MergePrToolRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let (workspace_id, repo_id) = match self
            .resolve_pr_scope(request.workspace_id, request.repo_id)
            .await
        {
            Ok(scope) => scope,
            Err(e) => return Ok(Self::tool_error(e)),
        };
        let url = self.url(&format!(
            "/api/workspaces/{workspace_id}/pull-requests/merge"
        ));
        let mut payload = serde_json::json!({
            "repo_id": repo_id,
            "pr": request.pr,
            "delete_branch": request.delete_branch.unwrap_or(false),
            "force": request.force.unwrap_or(false),
        });
        if let Some(method) = request.method {
            payload["method"] = method_name(&method).into();
        }
        match self
            .send_json::<Value>(self.client.post(&url).json(&payload))
            .await
        {
            Ok(value) => McpServer::success(&value),
            Err(e) => Ok(Self::tool_error(e)),
        }
    }

    #[tool(
        description = "Update a pull request: retitle, replace the body, and/or mark a draft ready for review (ready_for_review=true) or convert it back to a draft (false). Returns the PR state afterwards."
    )]
    async fn update_pr(
        &self,
        Parameters(request): Parameters<UpdatePrToolRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        if request.title.is_none() && request.body.is_none() && request.ready_for_review.is_none() {
            return McpServer::err(
                "Nothing to update: pass title, body or ready_for_review".to_string(),
                None,
            );
        }
        let (workspace_id, repo_id) = match self
            .resolve_pr_scope(request.workspace_id, request.repo_id)
            .await
        {
            Ok(scope) => scope,
            Err(e) => return Ok(Self::tool_error(e)),
        };
        let url = self.url(&format!(
            "/api/workspaces/{workspace_id}/pull-requests/update"
        ));
        let payload = serde_json::json!({
            "repo_id": repo_id,
            "pr": request.pr,
            "title": request.title,
            "body": request.body,
            "ready_for_review": request.ready_for_review,
        });
        match self
            .send_json::<Value>(self.client.post(&url).json(&payload))
            .await
        {
            Ok(value) => McpServer::success(&value),
            Err(e) => Ok(Self::tool_error(e)),
        }
    }
}

fn method_name(method: &MergeMethodParam) -> &'static str {
    match method {
        MergeMethodParam::Squash => "squash",
        MergeMethodParam::Merge => "merge",
        MergeMethodParam::Rebase => "rebase",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pr_number_is_read_from_pull_urls_only() {
        assert_eq!(
            pr_number_from_url("https://github.com/davidvasandani/homelab/pull/1267"),
            Some(1267)
        );
        assert_eq!(
            pr_number_from_url("https://github.com/o/r/pull/12/"),
            Some(12)
        );
        assert_eq!(pr_number_from_url("https://github.com/o/r/issues/12"), None);
        assert_eq!(pr_number_from_url("not a url"), None);
    }

    #[test]
    fn single_repo_is_the_default_and_many_repos_are_listed() {
        let one = [serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000001",
            "name": "vibe-kanban",
            "target_branch": "main"
        })];
        assert_eq!(
            select_single_repo(&one).unwrap(),
            "00000000-0000-0000-0000-000000000001"
                .parse::<Uuid>()
                .unwrap()
        );

        let two = [
            one[0].clone(),
            serde_json::json!({"id": "00000000-0000-0000-0000-000000000002", "name": "homelab"}),
        ];
        let error = select_single_repo(&two).unwrap_err();
        assert!(error.message.contains("repo_id is required"));
        let details = error.details.unwrap();
        assert!(details.contains("vibe-kanban (00000000-0000-0000-0000-000000000001)"));
        assert!(details.contains("homelab (00000000-0000-0000-0000-000000000002)"));

        assert!(select_single_repo(&[]).is_err());
    }
}
