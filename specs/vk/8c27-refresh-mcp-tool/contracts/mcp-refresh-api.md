# Contract: Workspace Session MCP Refresh

## REST request

`POST /api/workspaces/{workspace_id}/sessions/{session_id}/mcp/refresh`

No body is required. Workspace middleware and session ownership checks apply.

## Response

```json
{
  "success": true,
  "data": {
    "status": "pending_next_turn",
    "retryable": false,
    "generation": 3,
    "requested_at": "2026-07-27T12:00:00Z",
    "last_successful_refresh_at": "2026-07-27T11:40:00Z",
    "servers": []
  }
}
```

`status` is one of:

- `pending_next_turn`;
- `refreshed`;
- `partially_refreshed`;
- `busy`;
- `unsupported`;
- `failed`.

Busy and unsupported are domain results in the normal API envelope so web and
MCP callers receive the same structured contract. Missing workspace/session and
ownership violations use existing route errors.

Each `servers[]` item contains only:

- `server_id`;
- `status`;
- optional tool/resource/prompt counts;
- `restart_occurred` (`true`, `false`, or `null` for unknown);
- optional structured safe error.

## Status read

`GET /api/workspaces/{workspace_id}/sessions/{session_id}/mcp/refresh`

Returns the current process-local state so the UI can observe
`pending_next_turn -> refreshed` without treating request acknowledgement as
success.

## VK MCP tool

`refresh_mcp_tools`

Inputs:

- `workspace_id` and `session_id` are optional in orchestrator mode and default
  to the fixed MCP context;
- both are required in global mode.

The tool returns the same safe domain payload. Orchestrator inputs may narrow but
never widen fixed scope.
