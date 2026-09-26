use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool,
    tool_router,
};
use serde::{Deserialize, Serialize};

use super::McpServer;

/// How long `reauthenticate` watches a run: inside a 60 s tool deadline.
const WAIT_SECS: u32 = 50;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct McpReauthenticateRequest {
    #[schemars(
        description = "Target id from list_reauth_targets, e.g. `aws-sso:<session>`, `cli-tool:az`, `cli-tool:acli`, `sgsc:dp`. Omit to repair every expired target the sweep covers (sgsc backends must be named)."
    )]
    target: Option<String>,
}

#[derive(Debug, Serialize)]
struct RunBody<'a> {
    target: Option<&'a str>,
    wait_secs: u32,
    trigger: &'static str,
}

#[derive(Debug, Serialize)]
struct McpReauthenticateResponse {
    runs: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<&'static str>,
}

fn any_running(runs: &serde_json::Value) -> bool {
    runs.as_array().is_some_and(|runs| {
        runs.iter()
            .any(|r| r.pointer("/run/outcome").and_then(|o| o.as_str()) == Some("running"))
    })
}

#[tool_router(router = reauth_tools_router, vis = "pub")]
impl McpServer {
    #[tool(
        description = "List the credentials Vibe Kanban can re-authenticate unattended (AWS SSO scopes, Entra CLIs such as az, the Atlassian CLI, sgsc-mcp gateway backends), each with its current auth state and last run."
    )]
    async fn list_reauth_targets(&self) -> Result<CallToolResult, ErrorData> {
        let url = self.url("/api/reauth/targets");
        match self
            .send_json::<serde_json::Value>(self.client.get(&url))
            .await
        {
            Ok(overview) => McpServer::success(&overview),
            Err(e) => Ok(Self::tool_error(e)),
        }
    }

    #[tool(
        description = "Re-authenticate an expired credential with no human involved, then retry your command. Call it when a command fails with: AWS `The SSO session associated with this profile has expired` / `Error loading SSO Token` / `Token has expired and refresh failed`; az `InteractionRequired`, `TokenCreatedWithOutdatedPolicies`, `AADSTS50173`; acli `unauthorized: use 'acli confluence auth login'`; or an sgsc-mcp `reauth_required` / `Connect <provider> to continue` result (pass `sgsc:<backend>`, e.g. `sgsc:dp` for Snowflake, `sgsc:sf` for Salesforce). Waits up to 50 s; if a run is still `running`, poll list_reauth_targets instead of calling this again. A `refused` outcome needs the operator: do not retry it."
    )]
    async fn reauthenticate(
        &self,
        Parameters(McpReauthenticateRequest { target }): Parameters<McpReauthenticateRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url("/api/reauth/run");
        let body = RunBody {
            target: target.as_deref(),
            wait_secs: WAIT_SECS,
            trigger: "agent",
        };
        let runs: serde_json::Value = match self.send_json(self.client.post(&url).json(&body)).await
        {
            Ok(runs) => runs,
            Err(e) => return Ok(Self::tool_error(e)),
        };
        let hint = if any_running(&runs) {
            Some("Still running: call list_reauth_targets in a minute to see the outcome.")
        } else if runs.as_array().is_some_and(|r| r.is_empty()) {
            Some("Nothing to repair: no swept target is currently unauthenticated.")
        } else {
            None
        };
        McpServer::success(&McpReauthenticateResponse { runs, hint })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_is_detected_from_the_run_reports() {
        let runs = serde_json::json!([
            { "id": "cli-tool:az", "run": { "outcome": "succeeded" } },
            { "id": "aws-sso:sg", "run": { "outcome": "running" } },
        ]);
        assert!(any_running(&runs));
        assert!(!any_running(&serde_json::json!([
            { "id": "cli-tool:az", "run": { "outcome": "failed" } }
        ])));
    }

    #[test]
    fn agent_calls_always_identify_as_agent() {
        let body = serde_json::to_value(RunBody {
            target: Some("sgsc:dp"),
            wait_secs: WAIT_SECS,
            trigger: "agent",
        })
        .unwrap();
        assert_eq!(body["trigger"], "agent");
        assert!(body["wait_secs"].as_u64().unwrap() < 60);
    }
}
