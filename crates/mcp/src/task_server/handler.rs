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
        // Never show a route template here: spelling out the path teaches
        // agents to assemble one from loose ids, which is the dead-link failure
        // the omission of a parent issue URL exists to avoid.
        let issue_link_guidance = if has_get_issue {
            "In prose that is read inside Vibe Kanban (comments, session messages), write every \
             issue reference as a Markdown link whose destination is the issue_url or \
             related_issue_url a tool returned, copied verbatim, e.g. [VAS-646](<the returned \
             issue_url>) rather than a bare issue key. If a result has no URL, call 'get_issue' \
             for the issue you are naming."
        } else {
            "In prose that is read inside Vibe Kanban (session messages), link this workspace's \
             own issue using the issue_url returned by 'get_context', copied verbatim. This \
             server exposes no issue lookup, so name any other issue by its issue key instead of \
             linking it."
        };
        let mut instruction = format!(
            "{preamble} Use list/read tools first when you need IDs or current state. \
             {issue_link_guidance} Never assemble an issue URL yourself out of ids, an issue key \
             or the backend host. A returned issue_url is an application-relative Vibe Kanban \
             route, so it only resolves in the Vibe Kanban UI: do not put one anywhere the text \
             leaves the app, including pull request bodies, commit messages, Slack, email, or \
             fields mirrored to a linked external tracker (issue descriptions sync outbound to \
             Jira). Name the issue key there instead. TOOLS: {}.",
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

        // `contains("'get_issue'")` alone would be satisfied by the always
        // present `TOOLS:` listing, so match the instruction's own phrasing.
        let global = instructions(&McpServer::new_global("http://coordinator:3000"));
        assert!(global.contains("call 'get_issue'"), "{global}");

        // Orchestrator mode registers no remote-issue tools, so it must neither
        // name `get_issue` nor promise links for issues it cannot resolve.
        let orchestrator = instructions(&McpServer::new_orchestrator("http://coordinator:3000"));
        assert!(!orchestrator.contains("call 'get_issue'"), "{orchestrator}");
        assert!(
            orchestrator.contains("returned by 'get_context'"),
            "{orchestrator}"
        );
        assert!(orchestrator.contains("no issue lookup"), "{orchestrator}");
    }

    /// Spelling out the route would teach agents to build one from loose ids.
    #[test]
    fn issue_link_guidance_never_shows_a_route_template() {
        crate::task_server::tools::tests::install_rustls_provider();
        for server in [
            McpServer::new_global("http://coordinator:3000"),
            McpServer::new_orchestrator("http://coordinator:3000"),
        ] {
            let text = instructions(&server);
            assert!(!text.contains("/projects/"), "{text}");
            assert!(!text.contains("/issues/"), "{text}");
            assert!(text.contains("Never assemble an issue URL"), "{text}");
        }
    }

    #[test]
    fn issue_link_guidance_warns_against_pasting_relative_routes_externally() {
        crate::task_server::tools::tests::install_rustls_provider();
        let global = instructions(&McpServer::new_global("http://coordinator:3000"));
        assert!(global.contains("Markdown link"), "{global}");
        assert!(global.contains("leaves the app"), "{global}");
        // VK issue descriptions sync outbound to Jira, where the route 404s.
        assert!(global.contains("Jira"), "{global}");
        // The transport host must never be offered as a browser origin.
        assert!(!global.contains("coordinator:3000"), "{global}");
    }
}
