use rmcp::{
    ServerHandler,
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    tool_handler,
};

use super::{McpMode, McpServer};

// rmcp 1.8 defaults the router expression to `Self::tool_router()`; ours is a
// per-instance field selected by mode (global vs orchestrator).
#[tool_handler(router = self.tool_router)]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        let tools = self.tool_router.list_all();
        // Orchestrator mode registers no remote-issue tools, so the lookup we
        // point at has to match what is actually callable.
        let has_get_issue = tools.iter().any(|tool| tool.name.as_ref() == "get_issue");
        let mut tool_names = tools
            .into_iter()
            .map(|tool| format!("'{}'", tool.name))
            .collect::<Vec<_>>();
        tool_names.sort();

        let preamble = match self.mode() {
            McpMode::Global => {
                "A Vibe Kanban MCP server for task, issue, repository, workspace, and session management."
            }
            McpMode::Orchestrator => {
                "An orchestrator-scoped Vibe Kanban MCP server with tools limited to the configured workspace and orchestrator session context."
            }
        };
        let missing_url_lookup = if has_get_issue {
            "'get_issue'"
        } else {
            "'get_context'"
        };
        let mut instruction = format!(
            "{preamble} Use list/read tools first when you need IDs or current state. \
             Inside Vibe Kanban prose (issue descriptions, comments, session messages), write every \
             issue reference as a Markdown link to the returned issue_url or related_issue_url, \
             e.g. [VAS-646](/projects/<project_id>/issues/<issue_id>) — not a bare issue key. \
             If a result has no URL, call {missing_url_lookup}; never build one from an issue key or \
             the backend host. These are application-relative Vibe Kanban routes, so do not paste \
             them where an absolute URL is required (pull request bodies, Slack, email, commit \
             messages); name the issue key there instead. TOOLS: {}.",
            tool_names.join(", ")
        );
        if self.context.is_some() {
            instruction = format!(
                "Use 'get_context' to fetch project, issue, workspace, and orchestrator-session metadata for the active MCP context when available. {}",
                instruction
            );
        }

        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("vibe-kanban-mcp", "1.0.0"))
            .with_protocol_version(ProtocolVersion::V_2025_03_26)
            .with_instructions(instruction)
    }
}

#[cfg(test)]
mod tests {
    use rmcp::ServerHandler;

    use crate::task_server::McpServer;

    fn instructions(server: &McpServer) -> String {
        server.get_info().instructions.expect("instructions")
    }

    #[test]
    fn issue_link_guidance_points_at_a_lookup_that_mode_actually_registers() {
        crate::task_server::tools::tests::install_rustls_provider();

        let global = instructions(&McpServer::new_global("http://coordinator:3000"));
        assert!(global.contains("'get_issue'"), "{global}");

        // Orchestrator mode registers no remote-issue tools, so naming
        // `get_issue` there would send agents at a tool that does not exist.
        let orchestrator = instructions(&McpServer::new_orchestrator("http://coordinator:3000"));
        assert!(!orchestrator.contains("call 'get_issue'"), "{orchestrator}");
        assert!(
            orchestrator.contains("call 'get_context'"),
            "{orchestrator}"
        );
    }

    #[test]
    fn issue_link_guidance_warns_against_pasting_relative_routes_externally() {
        crate::task_server::tools::tests::install_rustls_provider();
        let global = instructions(&McpServer::new_global("http://coordinator:3000"));
        assert!(global.contains("Markdown link"), "{global}");
        assert!(global.contains("absolute URL is required"), "{global}");
        // The transport host must never be offered as a browser origin.
        assert!(!global.contains("coordinator:3000"), "{global}");
    }
}
