//! Workspace-scoped managed-browser tools: an agent adapter over the VK
//! browser-session REST API. The API stays the security boundary — these
//! tools add no authority of their own, workspace scope is fixed at MCP
//! launch, and every mutating call goes through the backend control arbiter.
//! High-frequency data (screencast frames, raw mouse movement) never flows
//! through MCP.

use base64::Engine;
use rmcp::{
    ErrorData,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content},
    schemars, tool, tool_router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::{McpServer, ToolError};

// ── Requests ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct BrowserCreateSessionRequest {
    #[schemars(description = "Workspace ID. Optional when running inside a scoped workspace MCP.")]
    workspace_id: Option<Uuid>,
    #[schemars(description = "Optional named browser profile (persistent cookies/storage)")]
    profile: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct BrowserListSessionsRequest {
    #[schemars(description = "Workspace ID. Optional when scoped.")]
    workspace_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct BrowserSessionRef {
    #[schemars(
        description = "Browser session ID. Optional: defaults to the workspace's open session."
    )]
    session_id: Option<Uuid>,
    #[schemars(description = "Workspace ID. Optional when scoped.")]
    workspace_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct BrowserAcquireControlRequest {
    #[schemars(description = "Browser session ID. Optional: defaults to the open session.")]
    session_id: Option<Uuid>,
    #[schemars(description = "Workspace ID. Optional when scoped.")]
    workspace_id: Option<Uuid>,
    #[schemars(
        description = "Execution ID to bind the lease to. Optional: defaults to the workspace's currently running coding-agent execution. Must belong to this workspace."
    )]
    execution_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct BrowserNavigateRequest {
    #[schemars(description = "URL to navigate to")]
    url: String,
    #[schemars(description = "Browser session ID. Optional: defaults to the open session.")]
    session_id: Option<Uuid>,
    #[schemars(description = "Workspace ID. Optional when scoped.")]
    workspace_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct BrowserClickRequest {
    #[schemars(description = "X coordinate in CSS pixels")]
    x: i32,
    #[schemars(description = "Y coordinate in CSS pixels")]
    y: i32,
    #[schemars(description = "Mouse button: left (default), middle, or right")]
    button: Option<String>,
    #[schemars(description = "Browser session ID. Optional: defaults to the open session.")]
    session_id: Option<Uuid>,
    #[schemars(description = "Workspace ID. Optional when scoped.")]
    workspace_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct BrowserTypeRequest {
    #[schemars(description = "Text to type into the focused element")]
    text: String,
    #[schemars(description = "Browser session ID. Optional: defaults to the open session.")]
    session_id: Option<Uuid>,
    #[schemars(description = "Workspace ID. Optional when scoped.")]
    workspace_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct BrowserKeyRequest {
    #[schemars(description = "Key to press (e.g. Enter, Tab, a)")]
    key: String,
    #[schemars(description = "Modifier keys: alt, ctrl, meta, shift")]
    modifiers: Option<Vec<String>>,
    #[schemars(description = "Browser session ID. Optional: defaults to the open session.")]
    session_id: Option<Uuid>,
    #[schemars(description = "Workspace ID. Optional when scoped.")]
    workspace_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct BrowserEvaluateRequest {
    #[schemars(description = "JavaScript expression to evaluate (privileged capability)")]
    expression: String,
    #[schemars(description = "Browser session ID. Optional: defaults to the open session.")]
    session_id: Option<Uuid>,
    #[schemars(description = "Workspace ID. Optional when scoped.")]
    workspace_id: Option<Uuid>,
}

// ── Responses ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct BrowserToolEnvelope<T: Serialize> {
    workspace_id: String,
    browser_session_id: Option<String>,
    #[schemars(description = "Current controller and generation after this call")]
    control: Option<serde_json::Value>,
    result: T,
}

// ── Helpers ─────────────────────────────────────────────────────────────

impl McpServer {
    /// Send a browser-session API request, surfacing the backend's typed
    /// error payload (code, controller, generation, retryable) in-band so
    /// agents can pause on CONTROL_LOST instead of crashing.
    async fn send_browser_json<T: serde::de::DeserializeOwned>(
        &self,
        rb: reqwest::RequestBuilder,
    ) -> Result<T, Box<CallToolResult>> {
        let resp = self
            .send_with_reconnect(rb)
            .await
            .map_err(|e| Box::new(Self::tool_error(e)))?;
        let status = resp.status();
        let body = resp.text().await.map_err(|e| {
            Box::new(Self::tool_error(ToolError::new(
                "Failed to read VK API response",
                Some(e.to_string()),
            )))
        })?;
        let envelope: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
            Box::new(Self::tool_error(ToolError::new(
                "Failed to parse VK API response",
                Some(e.to_string()),
            )))
        })?;
        if !status.is_success() || envelope["success"] != json!(true) {
            let message = envelope["message"].as_str().unwrap_or("");
            // Browser routes serialize the typed BrowserSessionError as the
            // message; pass it through structured when it parses.
            if let Ok(typed) = serde_json::from_str::<serde_json::Value>(message) {
                let code = typed["code"].as_str().unwrap_or("UNKNOWN").to_string();
                let retryable = matches!(
                    code.as_str(),
                    "CONTROL_LOST" | "CONTROL_CONFLICT" | "TIMEOUT"
                );
                let payload = json!({
                    "success": false,
                    "error": code,
                    "retryable": retryable,
                    "details": typed,
                });
                return Err(Box::new(CallToolResult::error(vec![Content::text(
                    serde_json::to_string_pretty(&payload)
                        .unwrap_or_else(|_| "Failed to serialize error".to_string()),
                )])));
            }
            return Err(Box::new(Self::tool_error(ToolError::new(
                format!("VK API returned error status: {status}"),
                Some(if message.is_empty() {
                    body
                } else {
                    message.to_string()
                }),
            ))));
        }
        serde_json::from_value(envelope["data"].clone()).map_err(|e| {
            Box::new(Self::tool_error(ToolError::new(
                "Failed to parse VK API response data",
                Some(e.to_string()),
            )))
        })
    }

    fn browser_workspace_id(&self, explicit: Option<Uuid>) -> Result<Uuid, Box<CallToolResult>> {
        let workspace_id = self
            .resolve_workspace_id(explicit)
            .map_err(|e| Box::new(Self::tool_error(e)))?;
        self.scope_allows_workspace(workspace_id)
            .map_err(|e| Box::new(Self::tool_error(e)))?;
        Ok(workspace_id)
    }

    /// Resolve the target session: explicit id, or the workspace's open
    /// session.
    async fn resolve_browser_session(
        &self,
        workspace_id: Uuid,
        explicit: Option<Uuid>,
    ) -> Result<Uuid, Box<CallToolResult>> {
        if let Some(id) = explicit {
            // An explicit session id must belong to the scoped workspace —
            // tool arguments can narrow but never widen the MCP scope.
            let url = self.url(&format!("/api/browser-sessions/{id}"));
            let session: serde_json::Value = self.send_browser_json(self.client.get(&url)).await?;
            let session_workspace = session["session"]["workspace_id"]
                .as_str()
                .and_then(|w| w.parse::<Uuid>().ok());
            if session_workspace != Some(workspace_id) {
                return Err(Box::new(Self::tool_error(ToolError::new(
                    "Browser session is outside the configured workspace scope",
                    Some(format!("session_id={id}")),
                ))));
            }
            return Ok(id);
        }
        let url = self.url(&format!(
            "/api/browser-sessions?workspace_id={workspace_id}"
        ));
        let sessions: Vec<serde_json::Value> =
            self.send_browser_json(self.client.get(&url)).await?;
        sessions
            .iter()
            .find(|s| !s["live"].is_null())
            .and_then(|s| s["session"]["id"].as_str())
            .and_then(|id| id.parse::<Uuid>().ok())
            .ok_or_else(|| {
                Box::new(Self::tool_error(ToolError::new(
                    "No open browser session for this workspace",
                    Some("Create one with browser_create_session".to_string()),
                )))
            })
    }

    async fn fetch_control(&self, session_id: Uuid) -> Option<serde_json::Value> {
        let url = self.url(&format!("/api/browser-sessions/{session_id}/control"));
        self.send_browser_json::<serde_json::Value>(self.client.get(&url))
            .await
            .ok()
    }

    fn browser_success<T: Serialize>(
        workspace_id: Uuid,
        session_id: Option<Uuid>,
        control: Option<&serde_json::Value>,
        result: T,
    ) -> Result<CallToolResult, ErrorData> {
        let envelope = BrowserToolEnvelope {
            workspace_id: workspace_id.to_string(),
            browser_session_id: session_id.map(|id| id.to_string()),
            control: control.cloned(),
            result,
        };
        Self::success(&envelope)
    }

    /// Execute a mutating action as the calling agent execution.
    /// Auto-acquires an uncontrolled session; never displaces a controller.
    async fn browser_agent_action(
        &self,
        workspace_id: Option<Uuid>,
        session_id: Option<Uuid>,
        action: serde_json::Value,
    ) -> Result<CallToolResult, ErrorData> {
        let workspace_id = match self.browser_workspace_id(workspace_id) {
            Ok(id) => id,
            Err(e) => return Ok(*e),
        };
        let session_id = match self.resolve_browser_session(workspace_id, session_id).await {
            Ok(id) => id,
            Err(e) => return Ok(*e),
        };
        let payload = json!({
            "as": "agent",
            "command_id": Uuid::new_v4(),
            "action": action,
            "auto_acquire": true,
        });
        let url = self.url(&format!("/api/browser-sessions/{session_id}/actions"));
        let result: serde_json::Value = match self
            .send_browser_json(self.client.post(&url).json(&payload))
            .await
        {
            Ok(r) => r,
            Err(e) => return Ok(*e),
        };
        let control = self.fetch_control(session_id).await;
        Self::browser_success(workspace_id, Some(session_id), control.as_ref(), result)
    }
}

// ── Tools ───────────────────────────────────────────────────────────────

#[tool_router(router = browser_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "Create a managed browser session for the workspace (or return the existing open one). The session is visible live in the VK Browser view and survives agent turns until closed or expired."
    )]
    async fn browser_create_session(
        &self,
        Parameters(request): Parameters<BrowserCreateSessionRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let workspace_id = match self.browser_workspace_id(request.workspace_id) {
            Ok(id) => id,
            Err(e) => return Ok(*e),
        };
        // Reuse the open session when present.
        if let Ok(existing) = self.resolve_browser_session(workspace_id, None).await {
            let control = self.fetch_control(existing).await;
            return Self::browser_success(
                workspace_id,
                Some(existing),
                control.as_ref(),
                json!({ "reused_existing": true }),
            );
        }
        let url = self.url("/api/browser-sessions");
        let payload = json!({ "workspace_id": workspace_id, "profile": request.profile });
        let session: serde_json::Value = match self
            .send_browser_json(self.client.post(&url).json(&payload))
            .await
        {
            Ok(s) => s,
            Err(e) => return Ok(*e),
        };
        let session_id = session["session"]["id"]
            .as_str()
            .and_then(|id| id.parse::<Uuid>().ok());
        let control = session["live"]["control"].clone();
        let control = if control.is_null() {
            None
        } else {
            Some(control)
        };
        Self::browser_success(workspace_id, session_id, control.as_ref(), session)
    }

    #[tool(
        description = "List the workspace's managed browser sessions with live status and controller state."
    )]
    async fn browser_list_sessions(
        &self,
        Parameters(request): Parameters<BrowserListSessionsRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let workspace_id = match self.browser_workspace_id(request.workspace_id) {
            Ok(id) => id,
            Err(e) => return Ok(*e),
        };
        let url = self.url(&format!(
            "/api/browser-sessions?workspace_id={workspace_id}&include_closed=true"
        ));
        let sessions: Vec<serde_json::Value> =
            match self.send_browser_json(self.client.get(&url)).await {
                Ok(s) => s,
                Err(e) => return Ok(*e),
            };
        Self::browser_success(workspace_id, None, None, sessions)
    }

    #[tool(
        description = "Get the control state (controller, generation, lease expiry) of a browser session."
    )]
    async fn browser_get_control(
        &self,
        Parameters(request): Parameters<BrowserSessionRef>,
    ) -> Result<CallToolResult, ErrorData> {
        let workspace_id = match self.browser_workspace_id(request.workspace_id) {
            Ok(id) => id,
            Err(e) => return Ok(*e),
        };
        let session_id = match self
            .resolve_browser_session(workspace_id, request.session_id)
            .await
        {
            Ok(id) => id,
            Err(e) => return Ok(*e),
        };
        let url = self.url(&format!("/api/browser-sessions/{session_id}/control"));
        let control: serde_json::Value = match self.send_browser_json(self.client.get(&url)).await {
            Ok(c) => c,
            Err(e) => return Ok(*e),
        };
        Self::browser_success(workspace_id, Some(session_id), Some(&control), json!({}))
    }

    #[tool(
        description = "Acquire browser control for the calling agent execution. Fails with CONTROL_CONFLICT if a human or another execution currently controls the session — agents never displace a live controller."
    )]
    async fn browser_acquire_control(
        &self,
        Parameters(request): Parameters<BrowserAcquireControlRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let workspace_id = match self.browser_workspace_id(request.workspace_id) {
            Ok(id) => id,
            Err(e) => return Ok(*e),
        };
        let session_id = match self
            .resolve_browser_session(workspace_id, request.session_id)
            .await
        {
            Ok(id) => id,
            Err(e) => return Ok(*e),
        };
        let url = self.url(&format!(
            "/api/browser-sessions/{session_id}/control/acquire"
        ));
        let payload = json!({ "as": "agent", "execution_id": request.execution_id });
        let control: serde_json::Value = match self
            .send_browser_json(self.client.post(&url).json(&payload))
            .await
        {
            Ok(c) => c,
            Err(e) => return Ok(*e),
        };
        Self::browser_success(workspace_id, Some(session_id), Some(&control), json!({}))
    }

    #[tool(description = "Release the calling agent execution's browser control lease.")]
    async fn browser_release_control(
        &self,
        Parameters(request): Parameters<BrowserSessionRef>,
    ) -> Result<CallToolResult, ErrorData> {
        let workspace_id = match self.browser_workspace_id(request.workspace_id) {
            Ok(id) => id,
            Err(e) => return Ok(*e),
        };
        let session_id = match self
            .resolve_browser_session(workspace_id, request.session_id)
            .await
        {
            Ok(id) => id,
            Err(e) => return Ok(*e),
        };
        let url = self.url(&format!(
            "/api/browser-sessions/{session_id}/control/release"
        ));
        let payload = json!({ "as": "agent" });
        let control: serde_json::Value = match self
            .send_browser_json(self.client.post(&url).json(&payload))
            .await
        {
            Ok(c) => c,
            Err(e) => return Ok(*e),
        };
        Self::browser_success(workspace_id, Some(session_id), Some(&control), json!({}))
    }

    #[tool(
        description = "Navigate the workspace browser to a URL. Auto-acquires an uncontrolled session for this execution; returns retryable CONTROL_LOST/CONTROL_CONFLICT if a human controls it."
    )]
    async fn browser_navigate(
        &self,
        Parameters(request): Parameters<BrowserNavigateRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        self.browser_agent_action(
            request.workspace_id,
            request.session_id,
            json!({ "type": "navigate", "url": request.url }),
        )
        .await
    }

    #[tool(description = "Click at viewport coordinates in the workspace browser (control-gated).")]
    async fn browser_click(
        &self,
        Parameters(request): Parameters<BrowserClickRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let button = match request.button.as_deref() {
            None | Some("left") => None,
            Some(other @ ("middle" | "right")) => Some(other.to_string()),
            Some(other) => {
                return Ok(Self::tool_error(ToolError::new(
                    "Invalid mouse button",
                    Some(format!("expected left|middle|right, got {other}")),
                )));
            }
        };
        self.browser_agent_action(
            request.workspace_id,
            request.session_id,
            json!({ "type": "click", "x": request.x, "y": request.y, "button": button }),
        )
        .await
    }

    #[tool(
        description = "Type text into the focused element in the workspace browser (control-gated)."
    )]
    async fn browser_type(
        &self,
        Parameters(request): Parameters<BrowserTypeRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        self.browser_agent_action(
            request.workspace_id,
            request.session_id,
            json!({ "type": "type", "text": request.text }),
        )
        .await
    }

    #[tool(
        description = "Press a key (with optional modifiers) in the workspace browser (control-gated)."
    )]
    async fn browser_key(
        &self,
        Parameters(request): Parameters<BrowserKeyRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        self.browser_agent_action(
            request.workspace_id,
            request.session_id,
            json!({ "type": "key", "key": request.key, "modifiers": request.modifiers }),
        )
        .await
    }

    #[tool(
        description = "Evaluate a JavaScript expression in the workspace browser (privileged capability; may be denied by configuration)."
    )]
    async fn browser_evaluate(
        &self,
        Parameters(request): Parameters<BrowserEvaluateRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        self.browser_agent_action(
            request.workspace_id,
            request.session_id,
            json!({ "type": "evaluate", "expression": request.expression }),
        )
        .await
    }

    #[tool(
        description = "Take a screenshot of the workspace browser (read-only; allowed for any observer)."
    )]
    async fn browser_screenshot(
        &self,
        Parameters(request): Parameters<BrowserSessionRef>,
    ) -> Result<CallToolResult, ErrorData> {
        let workspace_id = match self.browser_workspace_id(request.workspace_id) {
            Ok(id) => id,
            Err(e) => return Ok(*e),
        };
        let session_id = match self
            .resolve_browser_session(workspace_id, request.session_id)
            .await
        {
            Ok(id) => id,
            Err(e) => return Ok(*e),
        };
        let url = self.url(&format!("/api/browser-sessions/{session_id}/screenshot"));
        let resp = match self.send_with_reconnect(self.client.get(&url)).await {
            Ok(resp) => resp,
            Err(e) => return Ok(Self::tool_error(e)),
        };
        if !resp.status().is_success() {
            return Ok(Self::tool_error(ToolError::message(format!(
                "VK API returned error status: {}",
                resp.status()
            ))));
        }
        let bytes = match resp.bytes().await {
            Ok(b) => b,
            Err(e) => {
                return Ok(Self::tool_error(ToolError::new(
                    "Failed to read screenshot",
                    Some(e.to_string()),
                )));
            }
        };
        let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
        Ok(CallToolResult::success(vec![Content::image(
            encoded,
            "image/png".to_string(),
        )]))
    }

    #[tool(
        description = "Get the workspace browser's current URL, title, and console tail (read-only)."
    )]
    async fn browser_get_page(
        &self,
        Parameters(request): Parameters<BrowserSessionRef>,
    ) -> Result<CallToolResult, ErrorData> {
        let workspace_id = match self.browser_workspace_id(request.workspace_id) {
            Ok(id) => id,
            Err(e) => return Ok(*e),
        };
        let session_id = match self
            .resolve_browser_session(workspace_id, request.session_id)
            .await
        {
            Ok(id) => id,
            Err(e) => return Ok(*e),
        };
        let url = self.url(&format!("/api/browser-sessions/{session_id}/page"));
        let page: serde_json::Value = match self.send_browser_json(self.client.get(&url)).await {
            Ok(p) => p,
            Err(e) => return Ok(*e),
        };
        let control = self.fetch_control(session_id).await;
        Self::browser_success(workspace_id, Some(session_id), control.as_ref(), page)
    }
}
