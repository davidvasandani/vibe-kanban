# Contract: MCP Refresh UI Reconciliation

The HTTP contract is unchanged.

## Request reload

`POST /api/workspaces/{workspace_id}/sessions/{session_id}/mcp/refresh`

- A new supported request normally returns canonical `pending_next_turn`.
- A duplicate while canonical state is pending returns `busy` as a transient
  projection with the same generation.
- Terminal unsupported/failure results are rendered as returned.

## Read canonical status

`GET /api/workspaces/{workspace_id}/sessions/{session_id}/mcp/refresh`

- Returns the canonical stored `McpRefreshResult`, or `null` if none exists.
- The browser reads this on session entry, after `busy`, and at the poll interval
  while canonical state is pending.

## Client obligations

- Never apply a response unless its workspace/session key is still active.
- Never interpret `busy` as canonical stored status.
- Never report `refreshed` unless the backend returns it.
- Stop polling on null or terminal canonical state and on session change.
