# Prior knowledge: graceful degradation for refused executions

Read-only search of `wiki/INDEX.md` for worker, placement, affinity, 503,
retry and `error_data`. Relevant pages: `worker-execution-hardening.md`,
`vk-pollers.md`, `mcp-pr-tools-and-connection-notices.md`,
`frontend-linting.md`, `coordinator-restart-handoff.md`.

## Worker admission and placement (`worker-execution-hardening.md`)

- A worker refuses new dispatches with 503 when memory or IO PSI
  `full avg60` is at or above `VK_WORKER_ADMISSION_MAX_PRESSURE` (default 40).
  Replays of accepted work are answered before the check. Heartbeats carry
  `admission_refusing` in `resource_snapshot`.
- The scheduler adds +1000 to a refusing worker's score instead of excluding
  it. Placement is sticky, so an existing workspace keeps targeting its
  assigned worker even when another worker is healthy. Only new placement
  benefits from the penalty.
- A dispatch makes at most one immediate retry. That retry rarely helps with
  sustained pressure. Do not raise the threshold, and do not migrate a live
  session silently just to hide the refusal (`vk/bdd8-debug-vk-load`).
- `avg60` is a smoothed average. Recovery can take minutes, not seconds.
- Dispatch errors already name the server's hostname and UUID, and the
  persisted evidence includes `worker_hostname`/`worker_node_id`.

## Error contracts (`vk-pollers.md`, `mcp-pr-tools-and-connection-notices.md`)

- Any route an MCP tool can reach must use `error_with_data_and_message`:
  non-browser clients only surface `message`, so a typed `error_data` alone
  reaches them as "Unknown error".
- Test the contract that crosses the boundary (status, body), not only the
  internal enum.

## Affinity moves (`coordinator-restart-handoff.md`, source)

- `PATCH /api/workspaces/{id}/affinity` with `requested_worker_node_id` moves an
  idle workspace (including Codex rollout transfer). It refuses while scripts,
  dev servers or helpers run. Existing `Local` workspaces cannot move to a
  worker.
- The Server Affinity picker uses `isWorkerEligible` (online, healthy mount,
  live lease, executor-profile capability).

## Frontend conventions (`frontend-linting.md`)

- No ESLint directive comments. Every `exhaustive-deps` warning fails lint.
- A feature may not import `@/features/**`, including itself, so use relative
  imports within a feature. `.tsx` files are PascalCase unless they start with
  `use`.
- Read values that must not retrigger an effect through a ref.
