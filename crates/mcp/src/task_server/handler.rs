use rmcp::{
    ServerHandler,
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    tool_handler,
};

use super::{McpMode, McpServer};

/// What an agent does before abandoning a Vibe Kanban tool for an
/// out-of-band fallback (homelab principle 143). Both `refresh_mcp_tools` and
/// `restart_workspace` are registered in every mode, so each rung is callable;
/// the ladder only promises what they can fix (transport and tool discovery),
/// and errors the backend explained (`vk_reported`) skip it.
pub(crate) const RECOVERY_LADDER: &str = "If a Vibe Kanban tool call fails, read the error \
     first. With \"vk_reported\": true, Vibe Kanban itself answered and explained the failure \
     (authentication, an org token that is configured but unavailable, configuration, a merge \
     refusal, GitHub's own message): refreshing or restarting cannot fix that, so act on the \
     message (fix the cause, retry later when it says so, or fall back at once) and quote it in \
     your report. Climb this ladder only for a transport or tool-discovery failure (no \
     vk_reported: a connection error or timeout, a non-JSON or proxy response, an unknown tool, \
     or the harness saying a Vibe Kanban MCP server failed to connect): (1) retry the call; (2) \
     call 'refresh_mcp_tools' with workspace_id (your VK_WORKSPACE_ID environment variable; \
     optional in an orchestrator-scoped context) and, if the \
     workspace has several sessions, session_id from 'list_sessions' (an 'unsupported' result, \
     as for Claude Code, counts as done); (3) call 'restart_workspace' with workspace_id, which \
     keeps the worktree, Git state, sessions and conversations; (4) only then fall back (for \
     example `gh` instead of 'create_pr'/'get_pr'/'merge_pr'). Rungs 2 and 3 are Vibe Kanban \
     calls too: if the server itself is unreachable (every call gets a connection error or a \
     proxy error page such as a Cloudflare 5xx), go from (1) straight to (4). When you fall \
     back, say so in your final report, naming the rung you reached or the reported error. A \
     connection notice about a differently named entry (such as 'vibe-kanban' from a repository \
     .mcp.json) is not evidence that this server is down: a real tool call is the test. Open, \
     poll and merge pull requests with 'create_pr' (with head for another PR from a different \
     branch), 'get_pr'/'list_pr_checks' and 'merge_pr' rather than `gh`. A script that needs a \
     GitHub token gets the owner-routed one from `GH_REPO=<owner>/<repo> gh auth token`, never \
     from the ambient GH_TOKEN or GITHUB_TOKEN.";

/// An issue comment and a delivered message are different things; say which
/// tool does what so an agent does not report a saved note as delivered.
const COMMENT_DELIVERY_GUIDANCE: &str = "To answer an agent that is waiting on an issue, \
     call 'create_issue_comment' with intent 'reply_to_agent' and read the returned \
     delivery.state; intent 'note' only saves the comment. A saved comment is not a delivery.";

// rmcp 1.8 defaults the router expression to `Self::tool_router()`; ours is a
// per-instance field selected by mode (global vs orchestrator).
#[tool_handler(router = self.tool_router)]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        let tools = self.tool_router.list_all();
        // Orchestrator mode registers no remote-issue tools, so the lookup we
        // point at has to match what is actually callable.
        let has_get_issue = tools.iter().any(|tool| tool.name.as_ref() == "get_issue");
        let comment_guidance = if tools
            .iter()
            .any(|tool| tool.name.as_ref() == "create_issue_comment")
        {
            COMMENT_DELIVERY_GUIDANCE
        } else {
            ""
        };
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
             own issue using the issue_url returned by 'get_context', copied verbatim, when that \
             field holds a route — it is null for a workspace with no linked issue, and null is \
             never a link destination. This server exposes no issue lookup, so name every other \
             issue, and this one when the field is null, by its issue key instead of linking it."
        };
        let mut instruction = format!(
            "{preamble} Use list/read tools first when you need IDs or current state. \
             {issue_link_guidance} {comment_guidance} Never assemble an issue URL yourself out of ids, an issue key \
             or the backend host. A returned issue_url is an application-relative Vibe Kanban \
             route, so it only resolves in the Vibe Kanban UI: do not put one anywhere the text \
             leaves the app, including pull request bodies, commit messages, Slack, email, or \
             fields mirrored to a linked external tracker (issue titles and descriptions sync \
             outbound to Jira). Name the issue key there instead. {RECOVERY_LADDER} TOOLS: {}.",
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
    fn recovery_ladder_is_given_in_order_in_every_mode() {
        crate::task_server::tools::tests::install_rustls_provider();
        for server in [
            McpServer::new_global("http://coordinator:3000"),
            McpServer::new_orchestrator("http://coordinator:3000"),
        ] {
            let text = instructions(&server);
            let rungs = [
                "(1) retry the call",
                "(2) call 'refresh_mcp_tools'",
                "(3) call 'restart_workspace'",
                "(4) only then fall back",
            ];
            let positions: Vec<usize> = rungs
                .iter()
                .map(|rung| text.find(rung).unwrap_or_else(|| panic!("{rung}: {text}")))
                .collect();
            assert!(positions.windows(2).all(|w| w[0] < w[1]), "{text}");
            assert!(text.contains("keeps the worktree, Git state"), "{text}");
            assert!(text.contains("naming the rung you reached"), "{text}");
            // Explained errors skip the ladder; an unreachable server skips
            // the rungs that are themselves VK calls.
            let reported = text
                .find("\"vk_reported\": true")
                .expect("vk_reported rule");
            assert!(reported < positions[0], "{text}");
            assert!(
                text.contains("refreshing or restarting cannot fix that"),
                "{text}"
            );
            assert!(text.contains("go from (1) straight to (4)"), "{text}");
            // The rungs name the ids they need.
            assert!(
                text.contains("'refresh_mcp_tools' with workspace_id"),
                "{text}"
            );
            assert!(text.contains("session_id from 'list_sessions'"), "{text}");
            assert!(
                text.contains("GH_REPO=<owner>/<repo> gh auth token"),
                "{text}"
            );
            assert!(
                text.contains("not evidence that this server is down"),
                "{text}"
            );
            // Every tool the ladder names is actually registered in this mode.
            for tool in [
                "'refresh_mcp_tools'",
                "'restart_workspace'",
                "'create_pr'",
                "'merge_pr'",
                "'list_sessions'",
            ] {
                assert!(text.matches(tool).count() >= 2, "{tool} not listed: {text}");
            }
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
