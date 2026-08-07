# Refresh MCP Rematerialization

## Objective

Refresh MCP for an active remote Codex execution must resolve the coordinator's
latest settings-owned MCP map, atomically replace only the MCP section in that
execution's scoped Codex `config.toml`, and then ask the same live Codex process
to reload. On-disk success alone is never reported as live adoption.

## Contract

- The coordinator resolves the latest session executor/profile with the same
  cached profile and native MCP reader used by dispatch.
- Persisted `ExecutionWorkerJob` affinity selects the target worker and exact
  execution ID.
- The signed worker request carries a bounded `McpConfigSnapshot` and matching
  path/body execution identity.
- The worker serializes refresh per execution, edits only that execution's
  existing scoped config through the atomic agent-config writer, and invokes
  `config/mcpServer/reload` only after the write succeeds.
- Worker outcomes distinguish queued, busy, unsupported, materialization
  failure, and reload/bootstrap failure without returning definition values.
- The existing session generation remains pending until Codex status evidence
  confirms the next active inventory; refreshed and partially refreshed remain
  process-confirmed outcomes.

## Preservation and isolation

The scoped home is not recreated during refresh. Authentication, skills,
history, session files, symlinked runtime assets, non-MCP config, and the live
conversation remain intact. Execution-ID paths and per-job claims prevent
cross-session writes.

## Verification

Coverage must prove snapshot A becomes B for additions, updates, disables, and
removals; unrelated config survives; concurrent execution homes remain
isolated; overlapping refresh is busy; errors are phase-specific and
secret-safe; and a worker-side deterministic MCP initialize plus `tools/list`
sees B without changing conversation identity.

Detailed SpecKit artifacts live in `specs/vk/cc71-refresh-mcp-shou/`.
