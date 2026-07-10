use db::models::execution_process::ExecutionProcess;
use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool,
    tool_router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::McpServer;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct McpSpawnBackgroundHelperRequest {
    #[schemars(
        description = "Bash script to run as the background helper (e.g. a file watcher, tunnel, or log follower). It is spawned by Vibe Kanban in its own process group, so it survives the end of the current agent turn."
    )]
    script: String,
    #[schemars(
        description = "Optional directory to run the script in, relative to the workspace root. Must not be absolute or contain '..'."
    )]
    working_dir: Option<String>,
    #[schemars(
        description = "Workspace ID to spawn the helper in. Optional if running inside that workspace context."
    )]
    workspace_id: Option<Uuid>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct McpSpawnBackgroundHelperResponse {
    success: bool,
    #[schemars(
        description = "Execution process ID of the helper; pass to stop_background_helper to stop it"
    )]
    execution_process_id: String,
    #[schemars(description = "Status of the helper process (running)")]
    status: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct McpListBackgroundHelpersRequest {
    #[schemars(
        description = "Workspace ID to list helpers for. Optional if running inside that workspace context."
    )]
    workspace_id: Option<Uuid>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct BackgroundHelperSummary {
    #[schemars(description = "Execution process ID")]
    id: String,
    #[schemars(description = "Status of the helper process")]
    status: String,
    #[schemars(description = "The script the helper is running")]
    script: Option<String>,
    #[schemars(description = "Directory the helper runs in, relative to the workspace root")]
    working_dir: Option<String>,
    #[schemars(description = "When the helper was started")]
    started_at: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct McpListBackgroundHelpersResponse {
    helpers: Vec<BackgroundHelperSummary>,
    count: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct McpStopBackgroundHelperRequest {
    #[schemars(description = "Execution process ID of the helper to stop")]
    execution_process_id: Uuid,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct McpStopBackgroundHelperResponse {
    success: bool,
    execution_process_id: String,
}

fn script_details(process: &ExecutionProcess) -> (Option<String>, Option<String>) {
    use executors::actions::ExecutorActionType;
    match process.executor_action() {
        Ok(action) => match &action.typ {
            ExecutorActionType::ScriptRequest(script) => {
                (Some(script.script.clone()), script.working_dir.clone())
            }
            _ => (None, None),
        },
        Err(_) => (None, None),
    }
}

#[tool_router(router = background_helpers_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "Spawn a long-lived background helper process (file watcher, tunnel, log follower, ...) that survives the end of the current agent turn. Vibe Kanban tracks it: it appears in the Processes tab, keeps running across Vibe Kanban restarts, and can be stopped with stop_background_helper or from the UI. Use this instead of `setsid`/`nohup` tricks. `workspace_id` is optional if running inside that workspace context."
    )]
    async fn spawn_background_helper(
        &self,
        Parameters(McpSpawnBackgroundHelperRequest {
            script,
            working_dir,
            workspace_id,
        }): Parameters<McpSpawnBackgroundHelperRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let workspace_id = match self.resolve_workspace_id(workspace_id) {
            Ok(id) => id,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        if let Err(error) = self.scope_allows_workspace(workspace_id) {
            return Ok(Self::tool_error(error));
        }

        let url = self.url(&format!(
            "/api/workspaces/{}/execution/background-helpers/start",
            workspace_id
        ));
        let payload = serde_json::json!({
            "script": script,
            "working_dir": working_dir,
        });

        let process: ExecutionProcess =
            match self.send_json(self.client.post(&url).json(&payload)).await {
                Ok(process) => process,
                Err(e) => return Ok(Self::tool_error(e)),
            };

        McpServer::success(&McpSpawnBackgroundHelperResponse {
            success: true,
            execution_process_id: process.id.to_string(),
            status: Self::execution_process_status_label(&process.status).to_string(),
        })
    }

    #[tool(
        description = "List running background helper processes for a workspace. `workspace_id` is optional if running inside that workspace context."
    )]
    async fn list_background_helpers(
        &self,
        Parameters(McpListBackgroundHelpersRequest { workspace_id }): Parameters<
            McpListBackgroundHelpersRequest,
        >,
    ) -> Result<CallToolResult, ErrorData> {
        let workspace_id = match self.resolve_workspace_id(workspace_id) {
            Ok(id) => id,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        if let Err(error) = self.scope_allows_workspace(workspace_id) {
            return Ok(Self::tool_error(error));
        }

        let url = self.url(&format!(
            "/api/workspaces/{}/execution/background-helpers",
            workspace_id
        ));
        let processes: Vec<ExecutionProcess> = match self.send_json(self.client.get(&url)).await {
            Ok(processes) => processes,
            Err(e) => return Ok(Self::tool_error(e)),
        };

        let helpers: Vec<BackgroundHelperSummary> = processes
            .iter()
            .map(|process| {
                let (script, working_dir) = script_details(process);
                BackgroundHelperSummary {
                    id: process.id.to_string(),
                    status: Self::execution_process_status_label(&process.status).to_string(),
                    script,
                    working_dir,
                    started_at: process.started_at.to_rfc3339(),
                }
            })
            .collect();

        McpServer::success(&McpListBackgroundHelpersResponse {
            count: helpers.len(),
            helpers,
        })
    }

    #[tool(
        description = "Stop a running background helper process by its execution process ID (from spawn_background_helper or list_background_helpers)."
    )]
    async fn stop_background_helper(
        &self,
        Parameters(McpStopBackgroundHelperRequest {
            execution_process_id,
        }): Parameters<McpStopBackgroundHelperRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url(&format!(
            "/api/execution-processes/{}/stop",
            execution_process_id
        ));
        if let Err(e) = self.send_empty_json(self.client.post(&url)).await {
            return Ok(Self::tool_error(e));
        }

        McpServer::success(&McpStopBackgroundHelperResponse {
            success: true,
            execution_process_id: execution_process_id.to_string(),
        })
    }
}
