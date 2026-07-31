use std::str::FromStr;

use api_types::{
    Issue, ListIssuesResponse, ListOrganizationsResponse, ListProjectStatusesResponse,
    ListProjectsResponse, ProjectStatus, SearchIssuesRequest,
};
use db::models::{execution_process::ExecutionProcessStatus, tag::Tag};
use executors::executors::BaseCodingAgent;
use regex::Regex;
use rmcp::{
    ErrorData,
    model::{CallToolResult, Content},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;
use uuid::Uuid;

use super::{ApiResponseEnvelope, McpMode, McpServer};

type ToolCallResult = Result<CallToolResult, ErrorData>;

#[derive(Debug, Error)]
#[error("{message}")]
struct ToolError {
    message: String,
    details: Option<String>,
}

impl ToolError {
    fn new(message: impl Into<String>, details: Option<impl Into<String>>) -> Self {
        Self {
            message: message.into(),
            details: details.map(Into::into),
        }
    }

    fn message(message: impl Into<String>) -> Self {
        Self::new(message, None::<String>)
    }
}

/// How long to wait after a transient failure before retrying against the
/// re-resolved backend. Small enough to stay responsive, large enough to let a
/// backend that is mid-restart finish rebinding its port.
const RECONNECT_BACKOFF: std::time::Duration = std::time::Duration::from_millis(200);

/// Whether an error is a connection-level failure worth retrying after
/// re-resolving the backend URL (as opposed to, say, an HTTP error status,
/// which `send_with_reconnect` never sees). Covers the "backend moved ports"
/// and "backend mid-restart" cases that present as the MCP being unreachable.
fn is_transient_error(error: &reqwest::Error) -> bool {
    error.is_connect() || error.is_timeout()
}

/// Rewrites `request`'s scheme/host/port to point at `base_url`, preserving the
/// original path and query. Used to re-target a cloned request at the backend's
/// new address without rebuilding it from scratch.
fn retarget_request(request: &mut reqwest::Request, base_url: &str) -> Result<(), ToolError> {
    let base = reqwest::Url::parse(base_url)
        .map_err(|error| ToolError::new("Invalid backend URL", Some(error.to_string())))?;
    let url = request.url_mut();
    url.set_scheme(base.scheme())
        .map_err(|_| ToolError::message("Failed to set backend scheme"))?;
    url.set_host(base.host_str())
        .map_err(|error| ToolError::new("Invalid backend host", Some(error.to_string())))?;
    url.set_port(base.port())
        .map_err(|_| ToolError::message("Failed to set backend port"))?;
    Ok(())
}

mod background_helpers;
mod browser;
mod context;
mod issue_assignees;
mod issue_relationships;
mod issue_tags;
mod organizations;
mod remote_issues;
mod remote_projects;
mod repos;
mod sessions;
mod task_attempts;
mod workspaces;

impl McpServer {
    pub fn global_mode_router() -> rmcp::handler::server::tool::ToolRouter<Self> {
        Self::context_tools_router()
            + Self::workspaces_tools_router()
            + Self::background_helpers_tools_router()
            + Self::organizations_tools_router()
            + Self::repos_tools_router()
            + Self::remote_projects_tools_router()
            + Self::remote_issues_tools_router()
            + Self::issue_assignees_tools_router()
            + Self::issue_tags_tools_router()
            + Self::issue_relationships_tools_router()
            + Self::task_attempts_tools_router()
            + Self::session_tools_router()
            + Self::browser_tools_router()
    }

    pub fn orchestrator_mode_router() -> rmcp::handler::server::tool::ToolRouter<Self> {
        let mut router = Self::context_tools_router()
            + Self::workspaces_tools_router()
            + Self::background_helpers_tools_router()
            + Self::session_tools_router()
            + Self::browser_tools_router();
        router.remove_route("list_workspaces");
        router.remove_route("delete_workspace");
        router
    }
}

impl McpServer {
    fn orchestrator_session_id(&self) -> Option<Uuid> {
        self.context
            .as_ref()
            .and_then(|ctx| ctx.orchestrator_session_id)
    }

    fn scoped_workspace_id(&self) -> Option<Uuid> {
        self.context.as_ref().map(|ctx| ctx.workspace_id)
    }

    fn success<T: Serialize>(data: &T) -> ToolCallResult {
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(data)
                .unwrap_or_else(|_| "Failed to serialize response".to_string()),
        )]))
    }

    fn err<S: Into<String>>(msg: S, details: Option<S>) -> ToolCallResult {
        Ok(Self::tool_error(ToolError::new(msg, details)))
    }

    fn tool_error(error: ToolError) -> CallToolResult {
        let mut value = serde_json::json!({
            "success": false,
            "error": error.message,
        });
        if let Some(details) = error.details {
            value["details"] = serde_json::json!(details);
        }

        CallToolResult::error(vec![Content::text(
            serde_json::to_string_pretty(&value)
                .unwrap_or_else(|_| "Failed to serialize error".to_string()),
        )])
    }

    /// Sends a request, retrying once against a freshly re-resolved backend URL
    /// if the first attempt fails with a transient connection error.
    ///
    /// This is what makes a long-lived MCP session self-heal after the backend
    /// restarts on a different port: the cached URL points at a dead port, the
    /// first attempt fails to connect, we re-resolve (env vars → port file),
    /// re-target the request at the new host, and retry once.
    async fn send_with_reconnect(
        &self,
        rb: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, ToolError> {
        let request = rb.build().map_err(|error| {
            ToolError::new("Failed to build VK API request", Some(error.to_string()))
        })?;
        // Keep a clone so we can retry; JSON/empty bodies are always cloneable.
        let retry_request = request.try_clone();

        let first_error = match self.client.execute(request).await {
            Ok(resp) => return Ok(resp),
            Err(error) if is_transient_error(&error) => error,
            Err(error) => {
                return Err(ToolError::new(
                    "Failed to connect to VK API",
                    Some(error.to_string()),
                ));
            }
        };

        let Some(mut retry_request) = retry_request else {
            return Err(ToolError::new(
                "Failed to connect to VK API",
                Some(first_error.to_string()),
            ));
        };

        tracing::warn!(
            "VK API request to {} failed ({}); re-resolving backend URL and retrying",
            retry_request.url(),
            first_error
        );
        tokio::time::sleep(RECONNECT_BACKOFF).await;

        match self.refresh_base_url().await {
            Ok(base_url) => {
                if let Err(error) = retarget_request(&mut retry_request, &base_url) {
                    tracing::warn!(
                        "Failed to retarget VK API request after reconnect: {}",
                        error.message
                    );
                }
            }
            Err(error) => {
                tracing::warn!("Failed to re-resolve backend URL for reconnect: {}", error);
            }
        }

        self.client
            .execute(retry_request)
            .await
            .map_err(|error| ToolError::new("Failed to connect to VK API", Some(error.to_string())))
    }

    async fn send_json<T: DeserializeOwned>(
        &self,
        rb: reqwest::RequestBuilder,
    ) -> Result<T, ToolError> {
        let resp = self.send_with_reconnect(rb).await?;

        if !resp.status().is_success() {
            let status = resp.status();
            return Err(ToolError::message(format!(
                "VK API returned error status: {}",
                status
            )));
        }

        let api_response = resp
            .json::<ApiResponseEnvelope<T>>()
            .await
            .map_err(|error| {
                ToolError::new("Failed to parse VK API response", Some(error.to_string()))
            })?;

        if !api_response.success {
            let msg = api_response.message.as_deref().unwrap_or("Unknown error");
            return Err(ToolError::new("VK API returned error", Some(msg)));
        }

        api_response
            .data
            .ok_or_else(|| ToolError::message("VK API response missing data field"))
    }

    async fn send_empty_json(&self, rb: reqwest::RequestBuilder) -> Result<(), ToolError> {
        let resp = self.send_with_reconnect(rb).await?;

        if !resp.status().is_success() {
            let status = resp.status();
            return Err(ToolError::message(format!(
                "VK API returned error status: {}",
                status
            )));
        }

        #[derive(Deserialize)]
        struct EmptyApiResponse {
            success: bool,
            message: Option<String>,
        }

        let api_response = resp.json::<EmptyApiResponse>().await.map_err(|error| {
            ToolError::new("Failed to parse VK API response", Some(error.to_string()))
        })?;

        if !api_response.success {
            let msg = api_response.message.as_deref().unwrap_or("Unknown error");
            return Err(ToolError::new("VK API returned error", Some(msg)));
        }

        Ok(())
    }

    fn resolve_workspace_id(&self, explicit: Option<Uuid>) -> Result<Uuid, ToolError> {
        if let Some(id) = explicit {
            return Ok(id);
        }
        if let Some(workspace_id) = self.scoped_workspace_id() {
            return Ok(workspace_id);
        }
        Err(ToolError::message(
            "workspace_id is required (not available from current MCP context)",
        ))
    }

    fn scope_allows_workspace(&self, workspace_id: Uuid) -> Result<(), ToolError> {
        if matches!(self.mode(), McpMode::Orchestrator)
            && let Some(scoped_workspace_id) = self.scoped_workspace_id()
            && scoped_workspace_id != workspace_id
        {
            return Err(ToolError::new(
                "Operation is outside the configured workspace scope",
                Some(format!(
                    "requested workspace_id={}, configured workspace_id={}",
                    workspace_id, scoped_workspace_id
                )),
            ));
        }

        Ok(())
    }

    // Expands @tagname references in text by replacing them with tag content.
    async fn expand_tags(&self, text: &str) -> String {
        let tag_pattern = match Regex::new(r"@([^\s@]+)") {
            Ok(re) => re,
            Err(_) => return text.to_string(),
        };

        let tag_names: Vec<String> = tag_pattern
            .captures_iter(text)
            .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        if tag_names.is_empty() {
            return text.to_string();
        }

        let url = self.url("/api/tags");
        let tags: Vec<Tag> = match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<ApiResponseEnvelope<Vec<Tag>>>().await {
                    Ok(envelope) if envelope.success => envelope.data.unwrap_or_default(),
                    _ => return text.to_string(),
                }
            }
            _ => return text.to_string(),
        };

        let tag_map: std::collections::HashMap<&str, &str> = tags
            .iter()
            .map(|t| (t.tag_name.as_str(), t.content.as_str()))
            .collect();

        let result = tag_pattern.replace_all(text, |caps: &regex::Captures| {
            let tag_name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            match tag_map.get(tag_name) {
                Some(content) => (*content).to_string(),
                None => caps.get(0).map(|m| m.as_str()).unwrap_or("").to_string(),
            }
        });

        result.into_owned()
    }

    // Resolves a project_id from an explicit parameter or falls back to context.
    fn resolve_project_id(&self, explicit: Option<Uuid>) -> Result<Uuid, ToolError> {
        if let Some(id) = explicit {
            return Ok(id);
        }
        if let Some(ctx) = &self.context
            && let Some(id) = ctx.project_id
        {
            return Ok(id);
        }
        Err(ToolError::message(
            "project_id is required (not available from workspace context)",
        ))
    }

    /// Resolves an issue identifier that is either the internal UUID or the
    /// human-facing simple ID shown on the board (e.g. "VAS-64").
    ///
    /// Simple IDs are looked up in the workspace's linked project first; when
    /// there is no linked project or no match there, every project visible to
    /// the user is searched. A key matching issues in several projects is an
    /// error rather than a guess.
    async fn resolve_issue_id(&self, issue_id: &str) -> Result<Uuid, ToolError> {
        let trimmed = issue_id.trim();
        if let Ok(id) = Uuid::parse_str(trimmed) {
            return Ok(id);
        }

        let context_project_id = self.context.as_ref().and_then(|ctx| ctx.project_id);
        if let Some(project_id) = context_project_id
            && let Some(issue) = self.find_issue_by_simple_id(project_id, trimmed).await?
        {
            return Ok(issue.id);
        }

        let mut matches = Vec::new();
        for project_id in self.list_all_project_ids().await? {
            if Some(project_id) == context_project_id {
                continue;
            }
            if let Some(issue) = self.find_issue_by_simple_id(project_id, trimmed).await? {
                matches.push(issue.id);
            }
        }
        Self::select_unique_issue_match(&matches, trimmed)
    }

    fn select_unique_issue_match(matches: &[Uuid], simple_id: &str) -> Result<Uuid, ToolError> {
        match matches {
            [id] => Ok(*id),
            [] => Err(ToolError::message(format!(
                "No issue found with ID or simple ID '{simple_id}'. Use `list_issues` to look up issues."
            ))),
            _ => Err(ToolError::message(format!(
                "Simple ID '{simple_id}' matches issues in multiple projects. Pass the issue's UUID instead (see `list_issues`)."
            ))),
        }
    }

    async fn find_issue_by_simple_id(
        &self,
        project_id: Uuid,
        simple_id: &str,
    ) -> Result<Option<Issue>, ToolError> {
        let query = SearchIssuesRequest {
            project_id,
            status_id: None,
            status_ids: None,
            priority: None,
            parent_issue_id: None,
            search: None,
            simple_id: Some(simple_id.to_string()),
            assignee_user_id: None,
            tag_id: None,
            tag_ids: None,
            sort_field: None,
            sort_direction: None,
            limit: Some(1),
            offset: None,
        };
        let url = self.url("/api/remote/issues/search");
        let response: ListIssuesResponse =
            self.send_json(self.client.post(&url).json(&query)).await?;
        Ok(response.issues.into_iter().next())
    }

    async fn list_all_project_ids(&self) -> Result<Vec<Uuid>, ToolError> {
        let orgs_url = self.url("/api/organizations");
        let orgs: ListOrganizationsResponse = self.send_json(self.client.get(&orgs_url)).await?;

        let mut project_ids = Vec::new();
        for org in orgs.organizations {
            let url = self.url(&format!("/api/remote/projects?organization_id={}", org.id));
            let projects: ListProjectsResponse = self.send_json(self.client.get(&url)).await?;
            project_ids.extend(projects.projects.into_iter().map(|project| project.id));
        }
        Ok(project_ids)
    }

    // Resolves an organization_id from an explicit parameter or falls back to context.
    fn resolve_organization_id(&self, explicit: Option<Uuid>) -> Result<Uuid, ToolError> {
        if let Some(id) = explicit {
            return Ok(id);
        }
        if let Some(ctx) = &self.context
            && let Some(id) = ctx.organization_id
        {
            return Ok(id);
        }
        Err(ToolError::message(
            "organization_id is required (not available from workspace context)",
        ))
    }

    // Fetches project statuses for a project.
    async fn fetch_project_statuses(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<ProjectStatus>, ToolError> {
        let url = self.url(&format!(
            "/api/remote/project-statuses?project_id={}",
            project_id
        ));
        let response: ListProjectStatusesResponse = self.send_json(self.client.get(&url)).await?;
        Ok(response.project_statuses)
    }

    // Resolves a status name to status_id.
    async fn resolve_status_id(
        &self,
        project_id: Uuid,
        status_name: &str,
    ) -> Result<Uuid, ToolError> {
        let statuses = self.fetch_project_statuses(project_id).await?;
        statuses
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(status_name))
            .map(|s| s.id)
            .ok_or_else(|| {
                let available: Vec<&str> = statuses.iter().map(|s| s.name.as_str()).collect();
                ToolError::message(format!(
                    "Unknown status '{}'. Available statuses: {:?}",
                    status_name, available
                ))
            })
    }

    // Gets the default status_id for a project (first non-hidden status by sort_order).
    async fn default_status_id(&self, project_id: Uuid) -> Result<Uuid, ToolError> {
        let statuses = self.fetch_project_statuses(project_id).await?;
        statuses
            .iter()
            .filter(|s| !s.hidden)
            .min_by_key(|s| s.sort_order)
            .map(|s| s.id)
            .ok_or_else(|| ToolError::message("No visible statuses found for project"))
    }

    // Resolves a status_id to its display name. Falls back to UUID string if lookup fails.
    async fn resolve_status_name(&self, project_id: Uuid, status_id: Uuid) -> String {
        match self.fetch_project_statuses(project_id).await {
            Ok(statuses) => statuses
                .iter()
                .find(|s| s.id == status_id)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| status_id.to_string()),
            Err(_) => status_id.to_string(),
        }
    }

    // Links a workspace to a remote issue by fetching issue.project_id and calling link endpoint.
    async fn link_workspace_to_issue(
        &self,
        workspace_id: Uuid,
        issue_id: Uuid,
    ) -> Result<(), ToolError> {
        let issue_url = self.url(&format!("/api/remote/issues/{}", issue_id));
        let issue: Issue = self.send_json(self.client.get(&issue_url)).await?;

        let link_url = self.url(&format!("/api/workspaces/{}/links", workspace_id));
        let link_payload = serde_json::json!({
            "project_id": issue.project_id,
            "issue_id": issue_id,
        });
        self.send_empty_json(self.client.post(&link_url).json(&link_payload))
            .await
    }

    fn parse_executor_agent(executor: &str) -> Result<BaseCodingAgent, ToolError> {
        let normalized = executor.replace('-', "_").to_ascii_uppercase();
        BaseCodingAgent::from_str(&normalized)
            .map_err(|_| ToolError::message(format!("Unknown executor '{executor}'.")))
    }

    fn normalize_executor_name(executor: Option<&str>) -> Result<String, ToolError> {
        let Some(executor) = executor.map(str::trim).filter(|value| !value.is_empty()) else {
            return Ok("CODEX".to_string());
        };

        Self::parse_executor_agent(executor)
            .map(|agent| agent.to_string())
            .map_err(|_| {
                ToolError::message(format!(
                    "Unknown executor '{}' configured for session",
                    executor
                ))
            })
    }

    fn execution_process_status_label(status: &ExecutionProcessStatus) -> &'static str {
        match status {
            ExecutionProcessStatus::Running => "running",
            ExecutionProcessStatus::Completed => "completed",
            ExecutionProcessStatus::Failed => "failed",
            ExecutionProcessStatus::Killed => "killed",
            ExecutionProcessStatus::Interrupted => "interrupted",
            ExecutionProcessStatus::Indeterminate => "indeterminate",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, sync::Once};

    #[test]
    fn indeterminate_execution_status_has_a_stable_label() {
        assert_eq!(
            McpServer::execution_process_status_label(&ExecutionProcessStatus::Indeterminate),
            "indeterminate"
        );
    }

    use rmcp::handler::server::tool::ToolRouter;
    use uuid::Uuid;

    use super::{ExecutionProcessStatus, McpServer};
    use crate::task_server::{McpContext, McpMode, McpRepoContext};

    static RUSTLS_PROVIDER: Once = Once::new();

    fn install_rustls_provider() {
        RUSTLS_PROVIDER.call_once(|| {
            rustls::crypto::aws_lc_rs::default_provider()
                .install_default()
                .expect("Failed to install rustls crypto provider");
        });
    }

    fn tool_names(router: rmcp::handler::server::tool::ToolRouter<McpServer>) -> BTreeSet<String> {
        router
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect()
    }

    #[test]
    fn orchestrator_mode_exposes_only_scoped_workflow_tools() {
        let actual = tool_names(McpServer::orchestrator_mode_router());
        let expected = BTreeSet::from([
            "browser_acquire_control".to_string(),
            "browser_click".to_string(),
            "browser_create_session".to_string(),
            "browser_evaluate".to_string(),
            "browser_get_control".to_string(),
            "browser_get_page".to_string(),
            "browser_key".to_string(),
            "browser_list_sessions".to_string(),
            "browser_navigate".to_string(),
            "browser_release_control".to_string(),
            "browser_screenshot".to_string(),
            "browser_type".to_string(),
            "create_session".to_string(),
            "get_context".to_string(),
            "get_execution".to_string(),
            "list_background_helpers".to_string(),
            "list_sessions".to_string(),
            "refresh_mcp_tools".to_string(),
            "run_session_prompt".to_string(),
            "spawn_background_helper".to_string(),
            "stop_background_helper".to_string(),
            "update_session".to_string(),
            "update_workspace".to_string(),
        ]);

        assert_eq!(actual, expected);
    }

    #[test]
    fn global_mode_keeps_workspace_admin_and_discovery_tools() {
        let actual = tool_names(McpServer::global_mode_router());

        assert!(actual.contains("list_workspaces"));
        assert!(actual.contains("delete_workspace"));
        assert!(!actual.contains("output_markdown"));
    }

    #[test]
    fn orchestrator_session_id_is_resolved_from_context() {
        install_rustls_provider();
        let session_id = Uuid::new_v4();
        let workspace_id = Uuid::new_v4();
        let server = McpServer {
            client: reqwest::Client::new(),
            base_url: std::sync::Arc::new(std::sync::RwLock::new(
                "http://127.0.0.1:3000".to_string(),
            )),
            tool_router: ToolRouter::default(),
            context: Some(McpContext {
                organization_id: None,
                project_id: None,
                issue_id: None,
                orchestrator_session_id: Some(session_id),
                workspace_id,
                workspace_branch: "main".to_string(),
                workspace_repos: vec![McpRepoContext {
                    repo_id: Uuid::new_v4(),
                    repo_name: "repo".to_string(),
                    target_branch: "main".to_string(),
                }],
            }),
            mode: McpMode::Global,
        };

        assert_eq!(server.orchestrator_session_id(), Some(session_id));
        assert_eq!(server.resolve_workspace_id(None).unwrap(), workspace_id);
    }

    #[test]
    fn orchestrator_scope_requires_context_when_missing() {
        install_rustls_provider();
        let server = McpServer {
            client: reqwest::Client::new(),
            base_url: std::sync::Arc::new(std::sync::RwLock::new(
                "http://127.0.0.1:3000".to_string(),
            )),
            tool_router: ToolRouter::default(),
            context: None,
            mode: McpMode::Orchestrator,
        };

        assert_eq!(server.orchestrator_session_id(), None);
        assert!(server.resolve_workspace_id(None).is_err());
        assert!(server.scope_allows_workspace(Uuid::new_v4()).is_ok());
    }

    #[test]
    fn select_unique_issue_match_returns_single_match() {
        let issue_id = Uuid::new_v4();
        assert_eq!(
            McpServer::select_unique_issue_match(&[issue_id], "VAS-64").unwrap(),
            issue_id
        );
    }

    #[test]
    fn select_unique_issue_match_rejects_missing_key() {
        let error = McpServer::select_unique_issue_match(&[], "VAS-64").unwrap_err();
        assert!(error.to_string().contains("No issue found"));
    }

    #[test]
    fn select_unique_issue_match_rejects_ambiguous_key() {
        let error =
            McpServer::select_unique_issue_match(&[Uuid::new_v4(), Uuid::new_v4()], "VAS-64")
                .unwrap_err();
        assert!(error.to_string().contains("multiple projects"));
    }

    #[test]
    fn retarget_request_swaps_host_and_port_but_keeps_path_and_query() {
        install_rustls_provider();
        let client = reqwest::Client::new();
        let mut request = client
            .get("http://127.0.0.1:3000/api/organizations?scope=all")
            .build()
            .expect("request should build");

        super::retarget_request(&mut request, "http://127.0.0.1:4567")
            .expect("retarget should succeed");

        assert_eq!(
            request.url().as_str(),
            "http://127.0.0.1:4567/api/organizations?scope=all"
        );
    }

    #[test]
    fn global_context_omits_orchestrator_session_id_from_serialized_output() {
        install_rustls_provider();
        let context = McpContext {
            organization_id: None,
            project_id: None,
            issue_id: None,
            orchestrator_session_id: None,
            workspace_id: Uuid::new_v4(),
            workspace_branch: "main".to_string(),
            workspace_repos: vec![],
        };

        let serialized = serde_json::to_value(&context).expect("context should serialize");

        assert!(serialized.get("orchestrator_session_id").is_none());
    }
}
