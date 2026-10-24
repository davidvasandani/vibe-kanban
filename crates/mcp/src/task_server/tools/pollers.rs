use db::models::{
    execution_process::{ExecutionProcess, ExecutionProcessStatus},
    session::Session,
    wakeup::Wakeup,
};
use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool,
    tool_router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{McpMode, McpServer, ToolError};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct McpSpawnPollerRequest {
    #[schemars(
        description = "Command to run on each tick (e.g. `git fetch --dry-run origin main`). It is run by Vibe Kanban in its own process group, so it survives the end of the current agent turn."
    )]
    command: String,
    #[schemars(
        description = "Seconds between ticks. Required and never defaulted: must be between 5 and 86400."
    )]
    interval_secs: u32,
    /// Completion predicate checked before each tick; exit zero stops polling.
    stop_command: Option<String>,
    /// Prompt delivered to the creating session only when stop_command exits zero.
    on_stop_prompt: Option<String>,
    /// Session to resume; defaults to the originating session in orchestrator mode.
    session_id: Option<Uuid>,
    /// Positive total lifetime in seconds. Required unless stop_command is supplied.
    timeout_secs: Option<u32>,
    #[schemars(
        description = "Optional directory to run the command in, relative to the workspace root. Must not be absolute or contain '..'."
    )]
    working_dir: Option<String>,
    #[schemars(
        description = "Workspace ID to spawn the poller in. Optional if running inside that workspace context."
    )]
    workspace_id: Option<Uuid>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct McpSpawnPollerResponse {
    success: bool,
    #[schemars(description = "Execution process ID of the poller; pass to stop_poller to stop it")]
    execution_process_id: String,
    #[schemars(description = "Status of the poller process (running)")]
    status: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct McpListPollersRequest {
    #[schemars(
        description = "Workspace ID to list pollers for. Optional if running inside that workspace context."
    )]
    workspace_id: Option<Uuid>,
}

/// The wire shape of `GET /pollers`. Mirrors the server crate's
/// `ListPollersResponse` / `PollerSummary`, which this crate cannot depend on.
#[derive(Debug, Deserialize)]
struct ListPollersPayload {
    pollers: Vec<PollerPayload>,
}

#[derive(Debug, Deserialize)]
struct PollerPayload {
    id: Uuid,
    status: ExecutionProcessStatus,
    command: String,
    interval_secs: u32,
    /// Completion predicate checked before each tick; exit zero stops polling.
    stop_command: Option<String>,
    /// Positive total lifetime in seconds. Required unless stop_command is supplied.
    timeout_secs: Option<u32>,
    working_dir: Option<String>,
    started_at: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct PollerSummary {
    #[schemars(description = "Execution process ID")]
    id: String,
    #[schemars(description = "Status of the poller process")]
    status: String,
    #[schemars(description = "The command the poller runs on each tick")]
    command: String,
    #[schemars(description = "Seconds between ticks")]
    interval_secs: u32,
    /// Completion predicate checked before each tick; exit zero stops polling.
    stop_command: Option<String>,
    /// Positive total lifetime in seconds. Required unless stop_command is supplied.
    timeout_secs: Option<u32>,
    #[schemars(description = "Directory the poller runs in, relative to the workspace root")]
    working_dir: Option<String>,
    #[schemars(description = "When the poller was started")]
    started_at: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct McpListPollersResponse {
    pollers: Vec<PollerSummary>,
    count: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct McpStopPollerRequest {
    #[schemars(description = "Execution process ID of the poller to stop")]
    execution_process_id: Uuid,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct McpStopPollerResponse {
    success: bool,
    execution_process_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ScheduleWakeupRequest {
    /// Workspace ID; defaults to the current workspace.
    workspace_id: Option<Uuid>,
    /// Session ID; required in global mode, defaults to the originating session in orchestrator mode.
    session_id: Option<Uuid>,
    /// Timezone-aware RFC 3339 timestamp. Supply exactly one of fire_at or delay_secs.
    fire_at: Option<String>,
    /// Positive delay in seconds, at most 604800 (7 days).
    delay_secs: Option<u32>,
    /// Continuation prompt delivered to the same session when the timer fires.
    on_fire_prompt: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ListWakeupsRequest {
    /// Workspace ID; defaults to the current workspace.
    workspace_id: Option<Uuid>,
    /// Session ID; required in global mode, defaults to the originating session in orchestrator mode.
    session_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CancelWakeupRequest {
    workspace_id: Option<Uuid>,
    session_id: Option<Uuid>,
    /// Wake-up ID returned by schedule_wakeup or list_wakeups.
    wakeup_id: Uuid,
}

#[derive(Debug, Deserialize, Serialize)]
struct WakeupPayload {
    wakeup: Wakeup,
    delivery: serde_json::Value,
}

impl McpServer {
    /// Fail closed when scoped context is unavailable; never infer another session.
    fn resolve_wakeup_session(&self, explicit: Option<Uuid>) -> Result<Uuid, ToolError> {
        match self.mode() {
            McpMode::Global => {
                explicit.ok_or_else(|| ToolError::message("session_id is required in global mode"))
            }
            McpMode::Orchestrator => {
                let origin = self.orchestrator_session_id().ok_or_else(|| {
                    ToolError::message("Originating session is unavailable in MCP context")
                })?;
                if explicit.is_some_and(|id| id != origin) {
                    return Err(ToolError::message(
                        "Session is outside the configured MCP scope",
                    ));
                }
                Ok(origin)
            }
        }
    }

    async fn wakeup_target(
        &self,
        workspace_id: Option<Uuid>,
        session_id: Option<Uuid>,
    ) -> Result<(Uuid, Uuid), ToolError> {
        let workspace_id = self.resolve_workspace_id(workspace_id)?;
        self.scope_allows_workspace(workspace_id)?;
        let session_id = self.resolve_wakeup_session(session_id)?;
        let session: Session = self
            .send_json(
                self.client
                    .get(self.url(&format!("/api/sessions/{session_id}"))),
            )
            .await?;
        if session.workspace_id != workspace_id {
            return Err(ToolError::message("Session does not belong to workspace"));
        }
        Ok((workspace_id, session_id))
    }

    async fn fetch_wakeups(
        &self,
        workspace_id: Uuid,
        session_id: Uuid,
    ) -> Result<Vec<WakeupPayload>, ToolError> {
        let wakeups: Vec<WakeupPayload> = self
            .send_json(
                self.client
                    .get(self.url(&format!("/api/workspaces/{workspace_id}/execution/wakeups"))),
            )
            .await?;
        Ok(wakeups
            .into_iter()
            .filter(|entry| entry.wakeup.session_id == session_id)
            .collect())
    }
}

#[tool_router(router = pollers_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "Schedule a durable one-shot continuation on the originating agent session. Supply exactly one of fire_at (timezone-aware RFC 3339) or delay_secs, and on_fire_prompt. Maximum delay: 7 days; maximum 8 pending and 32 lifetime wake-ups per session (including poller wake-ups), so self-chaining is bounded. Survives turns and server restarts; overdue timers fire late with timing context. An active turn is never interrupted. Pending wake-ups are visible and cancellable in the workspace UI. Global callers must supply session_id."
    )]
    async fn schedule_wakeup(
        &self,
        Parameters(request): Parameters<ScheduleWakeupRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let (workspace_id, session_id) = match self
            .wakeup_target(request.workspace_id, request.session_id)
            .await
        {
            Ok(target) => target,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        let payload = serde_json::json!({
            "session_id": session_id,
            "fire_at": request.fire_at,
            "delay_secs": request.delay_secs,
            "on_fire_prompt": request.on_fire_prompt,
        });
        let wakeup: Wakeup = match self
            .send_json(
                self.client
                    .post(self.url(&format!("/api/workspaces/{workspace_id}/execution/wakeups")))
                    .json(&payload),
            )
            .await
        {
            Ok(value) => value,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        Self::success(&wakeup)
    }

    #[tool(
        description = "List durable timer and poller wake-ups and their delivery status for the originating session. Global callers must supply session_id."
    )]
    async fn list_wakeups(
        &self,
        Parameters(request): Parameters<ListWakeupsRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let (workspace_id, session_id) = match self
            .wakeup_target(request.workspace_id, request.session_id)
            .await
        {
            Ok(target) => target,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        match self.fetch_wakeups(workspace_id, session_id).await {
            Ok(wakeups) => {
                Self::success(&serde_json::json!({ "count": wakeups.len(), "wakeups": wakeups }))
            }
            Err(error) => Ok(Self::tool_error(error)),
        }
    }

    #[tool(
        description = "Cancel a pending wake-up belonging to the originating session. A successful cancellation guarantees it will not fire. Returns cancelled=false if delivery already started or it is no longer pending. Global callers must supply session_id."
    )]
    async fn cancel_wakeup(
        &self,
        Parameters(request): Parameters<CancelWakeupRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let (workspace_id, session_id) = match self
            .wakeup_target(request.workspace_id, request.session_id)
            .await
        {
            Ok(target) => target,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        let wakeups = match self.fetch_wakeups(workspace_id, session_id).await {
            Ok(value) => value,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        if !wakeups
            .iter()
            .any(|entry| entry.wakeup.id == request.wakeup_id)
        {
            return Self::err(
                "Wake-up is outside the configured session scope or does not exist",
                None,
            );
        }
        let cancelled: bool = match self
            .send_json(self.client.post(self.url(&format!(
                "/api/workspaces/{workspace_id}/execution/wakeups/{}/cancel",
                request.wakeup_id
            ))))
            .await
        {
            Ok(value) => value,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        Self::success(
            &serde_json::json!({ "wakeup_id": request.wakeup_id, "cancelled": cancelled }),
        )
    }

    #[tool(
        description = "Run a command on a repeating interval as a Vibe Kanban poller. It is spawned in its own process group and survives the end of the current agent turn and Vibe Kanban restarts. Use this instead of a background shell or the CLI's own monitor/watch tools, which are terminated when the turn ends. Vibe Kanban tracks it: it appears in the Processes tab and can be stopped with stop_poller or from the UI. `interval_secs` is required and never defaulted. You must supply a nonblank `stop_command` (checked before each tick; exit zero stops) or a positive `timeout_secs` (total lifetime including hung commands), or both. A manual stop action does not satisfy this requirement. Supply on_stop_prompt to resume the originating session when stop_command exits zero (not on timeout or manual stop). Use schedule_wakeup for a one-shot timer. `workspace_id` is optional if running inside that workspace context."
    )]
    async fn spawn_poller(
        &self,
        Parameters(McpSpawnPollerRequest {
            command,
            interval_secs,
            stop_command,
            timeout_secs,
            on_stop_prompt,
            session_id,
            working_dir,
            workspace_id,
        }): Parameters<McpSpawnPollerRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let workspace_id = match self.resolve_workspace_id(workspace_id) {
            Ok(id) => id,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        if let Err(error) = self.scope_allows_workspace(workspace_id) {
            return Ok(Self::tool_error(error));
        }

        let session_id = if on_stop_prompt.is_some() {
            match self.wakeup_target(Some(workspace_id), session_id).await {
                Ok((_, id)) => Some(id),
                Err(error) => return Ok(Self::tool_error(error)),
            }
        } else {
            None
        };
        let url = self.url(&format!(
            "/api/workspaces/{}/execution/pollers/start",
            workspace_id
        ));
        let payload = serde_json::json!({
            "command": command,
            "interval_secs": interval_secs,
            "stop_command": stop_command,
            "timeout_secs": timeout_secs,
            "on_stop_prompt": on_stop_prompt,
            "session_id": session_id,
            "working_dir": working_dir,
        });

        let process: ExecutionProcess =
            match self.send_json(self.client.post(&url).json(&payload)).await {
                Ok(process) => process,
                Err(e) => return Ok(Self::tool_error(e)),
            };

        McpServer::success(&McpSpawnPollerResponse {
            success: true,
            execution_process_id: process.id.to_string(),
            status: Self::execution_process_status_label(&process.status).to_string(),
        })
    }

    #[tool(
        description = "List running pollers for a workspace. `workspace_id` is optional if running inside that workspace context."
    )]
    async fn list_pollers(
        &self,
        Parameters(McpListPollersRequest { workspace_id }): Parameters<McpListPollersRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let workspace_id = match self.resolve_workspace_id(workspace_id) {
            Ok(id) => id,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        if let Err(error) = self.scope_allows_workspace(workspace_id) {
            return Ok(Self::tool_error(error));
        }

        let url = self.url(&format!(
            "/api/workspaces/{}/execution/pollers",
            workspace_id
        ));
        let payload: ListPollersPayload = match self.send_json(self.client.get(&url)).await {
            Ok(payload) => payload,
            Err(e) => return Ok(Self::tool_error(e)),
        };

        let pollers: Vec<PollerSummary> = payload
            .pollers
            .into_iter()
            .map(|poller| PollerSummary {
                id: poller.id.to_string(),
                status: Self::execution_process_status_label(&poller.status).to_string(),
                command: poller.command,
                interval_secs: poller.interval_secs,
                stop_command: poller.stop_command,
                timeout_secs: poller.timeout_secs,
                working_dir: poller.working_dir,
                started_at: poller.started_at,
            })
            .collect();

        McpServer::success(&McpListPollersResponse {
            count: pollers.len(),
            pollers,
        })
    }

    #[tool(
        description = "Stop a running poller by its execution process ID (from spawn_poller or list_pollers)."
    )]
    async fn stop_poller(
        &self,
        Parameters(McpStopPollerRequest {
            execution_process_id,
        }): Parameters<McpStopPollerRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        // Intentional parity with `stop_background_helper`: neither tool runs a
        // workspace-scope check before hitting the generic stop route, unlike
        // their spawn/list siblings. The gap is flagged for a separate decision
        // covering both tools rather than fixed on one side here.
        let url = self.url(&format!(
            "/api/execution-processes/{}/stop",
            execution_process_id
        ));
        if let Err(e) = self.send_empty_json(self.client.post(&url)).await {
            return Ok(Self::tool_error(e));
        }

        McpServer::success(&McpStopPollerResponse {
            success: true,
            execution_process_id: execution_process_id.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use executors::actions::script::{MAX_POLLER_INTERVAL_SECS, MIN_POLLER_INTERVAL_SECS};

    #[test]
    fn parses_stopping_rules_and_legacy_list_metadata() {
        let request: super::McpSpawnPollerRequest = serde_json::from_value(serde_json::json!({
            "command": "echo tick", "interval_secs": 60,
            "stop_command": "test -f done", "timeout_secs": 300
        }))
        .unwrap();
        assert_eq!(request.stop_command.as_deref(), Some("test -f done"));
        assert_eq!(request.timeout_secs, Some(300));
        let legacy: super::ListPollersPayload = serde_json::from_value(serde_json::json!({
            "pollers": [{"id": uuid::Uuid::new_v4(), "status": "running",
                "command": "echo tick", "interval_secs": 60, "working_dir": null,
                "started_at": "2026-09-15T00:00:00Z"}]
        }))
        .unwrap();
        assert!(legacy.pollers[0].stop_command.is_none());
        assert!(legacy.pollers[0].timeout_secs.is_none());
    }

    #[test]
    fn wakeup_schema_requires_prompt_but_not_interval_or_command() {
        let schema =
            serde_json::to_value(schemars::schema_for!(super::ScheduleWakeupRequest)).unwrap();
        assert_eq!(schema["required"], serde_json::json!(["on_fire_prompt"]));
        assert!(schema["properties"].get("command").is_none());
        assert!(schema["properties"].get("interval_secs").is_none());
        for field in ["fire_at", "delay_secs", "session_id", "workspace_id"] {
            assert!(schema["properties"].get(field).is_some());
        }
        let request: super::ScheduleWakeupRequest = serde_json::from_value(serde_json::json!({
            "fire_at": "2026-10-25T22:05:00-04:00", "on_fire_prompt": "Check service hours before proceeding"
        })).unwrap();
        assert_eq!(
            request.fire_at.as_deref(),
            Some("2026-10-25T22:05:00-04:00")
        );
        assert!(request.delay_secs.is_none());
        assert!(
            serde_json::from_value::<super::ScheduleWakeupRequest>(
                serde_json::json!({"delay_secs": 60})
            )
            .is_err()
        );
    }

    #[test]
    fn poller_continuation_is_optional_and_schema_exposes_it() {
        let schema =
            serde_json::to_value(schemars::schema_for!(super::McpSpawnPollerRequest)).unwrap();
        assert!(schema["properties"].get("on_stop_prompt").is_some());
        let request: super::McpSpawnPollerRequest = serde_json::from_value(serde_json::json!({
            "command": "echo tick", "interval_secs": 60, "stop_command": "test -f done",
            "on_stop_prompt": "Inspect the completed result"
        }))
        .unwrap();
        assert_eq!(
            request.on_stop_prompt.as_deref(),
            Some("Inspect the completed result")
        );
        assert!(request.session_id.is_none());
    }

    #[test]
    fn wakeup_session_scope_fails_closed_and_global_requires_explicit_session() {
        use crate::task_server::{McpContext, McpServer};
        super::super::tests::install_rustls_provider();
        let session_id = uuid::Uuid::new_v4();
        let mut server = McpServer::new_orchestrator("http://127.0.0.1:3000");
        assert!(server.resolve_wakeup_session(Some(session_id)).is_err());
        server.context = Some(McpContext {
            organization_id: None,
            project_id: None,
            issue_id: None,
            issue_url: None,
            orchestrator_session_id: Some(session_id),
            workspace_id: uuid::Uuid::new_v4(),
            workspace_branch: "main".into(),
            workspace_repos: vec![],
        });
        assert_eq!(server.resolve_wakeup_session(None).unwrap(), session_id);
        assert_eq!(
            server.resolve_wakeup_session(Some(session_id)).unwrap(),
            session_id
        );
        assert!(
            server
                .resolve_wakeup_session(Some(uuid::Uuid::new_v4()))
                .is_err()
        );
        server.mode = super::McpMode::Global;
        assert!(server.resolve_wakeup_session(None).is_err());
        assert_eq!(
            server.resolve_wakeup_session(Some(session_id)).unwrap(),
            session_id
        );
    }

    /// `spawn_poller`'s `interval_secs` description quotes these bounds as
    /// literals, because a schemars description must be a string literal. If the
    /// server's validated range moves, the description has to move with it.
    #[test]
    fn the_documented_interval_bounds_match_the_validated_ones() {
        assert_eq!(MIN_POLLER_INTERVAL_SECS, 5);
        assert_eq!(MAX_POLLER_INTERVAL_SECS, 86_400);
    }
}
