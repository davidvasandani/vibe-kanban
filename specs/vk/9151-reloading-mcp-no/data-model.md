# Data Model: Reliable MCP Reload

No database schema changes are required.

## Canonical refresh state

`McpRefreshResult`, returned by the existing API, remains authoritative:

- `status`: pending, terminal success/partial, busy projection, unsupported, or
  failure;
- `generation`: monotonically increasing session-scoped request generation;
- `requested_at`: ordering boundary for execution adoption;
- `last_successful_refresh_at`: advances only on complete confirmation;
- `servers`: atomic confirmed inventory snapshot;
- `error` and `retryable`: sanitized operator guidance.

## Client view state

The feature-local hook holds:

- the currently selected `(workspace_id, session_id)` key;
- the latest canonical `McpRefreshResult | null` for that key;
- a request-in-flight boolean.

The selected key is not persisted. Changing it invalidates visible state and
late responses from prior keys.

## State transitions

- session selected → clear old view → GET canonical state;
- reload clicked → POST;
- POST pending → store pending → poll GET;
- POST busy → show already-in-progress feedback → GET canonical state;
- GET pending → retain and continue polling;
- GET terminal/null → store result/null and stop polling;
- session changed → reject all late results for the former key.
