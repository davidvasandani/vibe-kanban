use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool,
    tool_router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::McpServer;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CreatePreviewLeaseRequest {
    #[schemars(
        description = "The single sandbox-local HTTP dev-server port to expose (1024-65535)."
    )]
    port: u16,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct PreviewLeaseSummary {
    id: Uuid,
    port: u16,
    expires_at: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct CreatePreviewLeaseResponse {
    lease: PreviewLeaseSummary,
    url: String,
    vite_host: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct StopPreviewLeaseRequest {
    lease_id: Uuid,
}

#[tool_router(router = preview_leases_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "Create a four-hour, one-port URL capability that lets the existing remote Firecrawl browser reach a sandbox-local HTTP dev server. Start the server first and bind it to 127.0.0.1. The returned URL is shown only once; stop it when verification finishes."
    )]
    async fn create_preview_lease(
        &self,
        Parameters(CreatePreviewLeaseRequest { port }): Parameters<CreatePreviewLeaseRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let workspace_id = match self.resolve_workspace_id(None) {
            Ok(id) => id,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        if let Err(error) = self.scope_allows_workspace(workspace_id) {
            return Ok(Self::tool_error(error));
        }
        let url = self.url(&format!(
            "/api/workspaces/{workspace_id}/execution/preview-leases"
        ));
        let response: CreatePreviewLeaseResponse = match self
            .send_json(
                self.client
                    .post(url)
                    .json(&serde_json::json!({ "port": port })),
            )
            .await
        {
            Ok(response) => response,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        Self::success(&response)
    }

    #[tool(
        description = "List active remote-browser preview leases for the current sandbox workspace. Capability URLs are intentionally never returned by this listing."
    )]
    async fn list_preview_leases(&self) -> Result<CallToolResult, ErrorData> {
        let workspace_id = match self.resolve_workspace_id(None) {
            Ok(id) => id,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        if let Err(error) = self.scope_allows_workspace(workspace_id) {
            return Ok(Self::tool_error(error));
        }
        let url = self.url(&format!(
            "/api/workspaces/{workspace_id}/execution/preview-leases"
        ));
        let leases: Vec<PreviewLeaseSummary> = match self.send_json(self.client.get(url)).await {
            Ok(leases) => leases,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        Self::success(&leases)
    }

    #[tool(
        description = "Revoke a remote-browser preview lease in the current sandbox and stop its tracked forwarding helper. Repeated calls are safe."
    )]
    async fn stop_preview_lease(
        &self,
        Parameters(StopPreviewLeaseRequest { lease_id }): Parameters<StopPreviewLeaseRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let workspace_id = match self.resolve_workspace_id(None) {
            Ok(id) => id,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        if let Err(error) = self.scope_allows_workspace(workspace_id) {
            return Ok(Self::tool_error(error));
        }
        let url = self.url(&format!(
            "/api/workspaces/{workspace_id}/execution/preview-leases/{lease_id}/stop"
        ));
        let lease: PreviewLeaseSummary = match self.send_json(self.client.post(url)).await {
            Ok(lease) => lease,
            Err(error) => return Ok(Self::tool_error(error)),
        };
        Self::success(&lease)
    }
}
