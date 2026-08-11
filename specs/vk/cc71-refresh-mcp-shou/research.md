# Research: Refresh Active Remote MCP Snapshots

## Existing behavior

- `LocalContainerService::refresh_mcp_tools` resolves only the latest base
  executor, opens a session generation, and queues the local in-memory Codex
  control when present. A remote execution has no control in that coordinator
  map, so it returns pending without changing worker configuration.
- VAS-356 builds `McpConfigSnapshot` from the coordinator's resolved coding
  agent profile at dispatch, sends it in the signed digest-covered dispatch,
  and materializes it into
  `$TMPDIR/vibe-kanban/mcp-config/<execution-id>/codex/config.toml`.
- `prepare_scoped_codex_home` symlinks every non-config runtime asset from the
  worker's native Codex home, while config is an isolated file. The prepared
  object owns cleanup for the execution lifetime.
- `write_coding_agent_mcp_servers_to_path` reads the existing/source config,
  replaces the configured MCP path, and uses the repository's atomic writer.
- Worker executor spawn currently maps `SpawnedChild` to `.child`, dropping the
  `mcp_refresh` signal that local execution registers.

## Decisions

### Reuse the dispatch snapshot resolver

The coordinator is the only owner of user settings and custom profile variants.
Refresh must use the same `ExecutorConfigs::get_cached().get_coding_agent(...)`
and `read_coding_agent_mcp_servers(...)` path as dispatch. Reading worker-global
config or modifying the old snapshot would violate settings-only authority.

### Refresh the live worker job

An execution-ID-scoped worker route preserves persisted affinity and makes the
worker the only writer to its scoped config. Re-dispatching the execution is not
appropriate: request digest idempotency deliberately rejects changed input for
an existing execution and a second execution would not control the current
conversation.

### Retain the app-server control

The Codex app-server refresh handle is the only repository-supported evidence
channel for live adoption. A standalone MCP initialize/tools-list probe is useful
as worker smoke coverage but cannot replace the live control in production.

### Replace only the MCP section

Re-running `prepare_scoped_codex_home` would delete and recreate an execution
root and risks disturbing the active process. Updating the existing config file
through the MCP adapter preserves all unrelated config and symlinked assets and
already provides staged atomic rename semantics.

### Layered error categories

Materialization failure happens before the Codex protocol call and remains safe
to retry with the old live configuration. Reload/bootstrap failure happens after
the new file is durable but before adoption is confirmed. These must be distinct
because remediation and actual on-disk state differ.

## Alternatives rejected

- Restarting every remote Codex session: loses or forks live session semantics
  and violates the requested in-place behavior where safe support exists.
- Updating worker-global `~/.codex/config.toml`: breaks settings ownership and
  concurrent-session isolation.
- Mutating the old dispatch snapshot: cannot reflect settings changes made
  after session start.
- Reporting success from atomic write alone: configuration on disk is not proof
  of live capability adoption.
- Adding a new serialization/config dependency: existing helpers already cover
  TOML section replacement and atomic writes.
