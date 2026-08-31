use db::models::execution_process::{ExecutionProcess, ExecutionProcessStatus};
use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool,
    tool_router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::McpServer;

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

#[tool_router(router = pollers_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "Run a command on a repeating interval as a Vibe Kanban poller. It is spawned in its own process group and survives the end of the current agent turn and Vibe Kanban restarts. Use this instead of a background shell or the CLI's own monitor/watch tools, which are terminated when the turn ends. Vibe Kanban tracks it: it appears in the Processes tab and can be stopped with stop_poller or from the UI. `interval_secs` is required and never defaulted. `workspace_id` is optional if running inside that workspace context."
    )]
    async fn spawn_poller(
        &self,
        Parameters(McpSpawnPollerRequest {
            command,
            interval_secs,
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

        let url = self.url(&format!(
            "/api/workspaces/{}/execution/pollers/start",
            workspace_id
        ));
        let payload = serde_json::json!({
            "command": command,
            "interval_secs": interval_secs,
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

    /// `spawn_poller`'s `interval_secs` description quotes these bounds as
    /// literals, because a schemars description must be a string literal. If the
    /// server's validated range moves, the description has to move with it.
    #[test]
    fn the_documented_interval_bounds_match_the_validated_ones() {
        assert_eq!(MIN_POLLER_INTERVAL_SECS, 5);
        assert_eq!(MAX_POLLER_INTERVAL_SECS, 86_400);
    }
}
