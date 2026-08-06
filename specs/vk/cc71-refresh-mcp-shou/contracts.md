# Contracts: Refresh Active Remote MCP Snapshots

## Coordinator to worker

`POST /v1/executions/{execution_id}/mcp/refresh`

Request fields:

- normal signed `RequestAuthority`;
- `execution_id`, which must equal the path and a worker-owned live job;
- fresh bounded `McpConfigSnapshot`, whose executor must match the job.

The coordinator routes to the worker recorded for the target execution. The
worker rejects path/body mismatch, unknown jobs, executor mismatch, oversized
snapshots, terminal jobs, and legacy jobs without a scoped config.

## Worker ordering

For one accepted request the worker performs:

1. claim the execution refresh slot;
2. validate snapshot identity and bound;
3. atomically replace only the scoped config's MCP section;
4. invoke live Codex `config/mcpServer/reload`;
5. release the claim and return phase outcome.

Step 4 never runs if step 3 fails. Concurrent requests never interleave steps
3-4. Other executions use distinct paths and claims.

## Outcomes

- `queued`: materialization succeeded and Codex acknowledged the reload request;
  final adoption remains pending until next-turn server status confirmation.
- `busy`: another refresh owns the execution claim; retryable.
- `unsupported`: no safe scoped/live refresh path exists; not successful.
- `materialization_failed`: scoped config replacement failed before reload;
  retryability is reported without path/content disclosure.
- `reload_failed`: live control was unavailable or Codex rejected/bootstrap
  failed after materialization; not successful.

No response or log includes MCP definitions, environment values, authenticated
URLs, tokens, or secret-bearing arguments.

## UI mapping

- worker `queued` -> existing `pending_next_turn` generation;
- confirmed all-ready status -> `refreshed`;
- confirmed mixed retained/unavailable status -> `partially_refreshed`;
- worker/session contention -> `busy`;
- worker `unsupported`/version skew -> `unsupported`;
- materialization/reload failure -> `failed` with distinct safe category/copy.
