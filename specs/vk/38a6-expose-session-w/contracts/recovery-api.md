# Contract: MCP Recovery API

All routes return the existing `ApiResponse<T>` envelope. Errors include an
actionable public `message` and typed safe data where applicable.

## Session restart

`POST /api/workspaces/{workspace_id}/sessions/{session_id}/mcp/restart`

Request:

```json
{}
```

Response data: `McpRecoveryResult` with `scope: "session"` and status
`accepted`, `in_progress`, or an immediate terminal result. The server resolves
the continuation executor configuration and message; callers cannot inject a
different command through this endpoint.

`GET /api/workspaces/{workspace_id}/sessions/{session_id}/mcp/restart`

Returns the current/last recovery generation or `null`.

## Workspace restart

`POST /api/workspaces/{workspace_id}/mcp/restart`

Request:

```json
{
  "resume_session_id": "optional UUID",
  "confirmed_running_restart": false
}
```

The session defaults to the scoped orchestrator session in the MCP tool. HTTP
callers provide it explicitly when a coding-agent continuation is required.
Running-session teardown requires explicit confirmation; the MCP restart tool
sets it because invoking that tool is the confirmation action.

`GET /api/workspaces/{workspace_id}/mcp/restart`

Returns the current/last workspace recovery generation or `null`.

## Refresh

Existing endpoint:

`POST /api/workspaces/{workspace_id}/sessions/{session_id}/mcp/refresh`

The response remains `McpRefreshResult`, with `unsupported` as an immediate
terminal status when the selected executor has no verified live-refresh
control. `CLAUDE_CODE` is unsupported. Remediation points to session restart.

## MCP tools

### `restart_session`

Input:

```json
{
  "workspace_id": "optional UUID",
  "session_id": "optional UUID"
}
```

Both identifiers default from orchestrator context. Global context requires
them. The result is `McpRecoveryResult`.

### `restart_workspace`

Input:

```json
{
  "workspace_id": "optional UUID",
  "session_id": "optional UUID to resume"
}
```

The workspace and requesting session default from orchestrator context. Global
callers must identify the workspace; session is optional if no continuation is
required. The result is `McpRecoveryResult`.

## Compatibility

New worker-protocol snapshot fields are optional/defaulted. An older worker or
an executor without runtime inventory produces a named `unsupported` terminal
state, never a successful empty inventory.
