# Research: Refresh Active Workspace MCP Inventories

## Current implementation

Vibe Kanban contains two related paths:

1. `POST /api/workspaces/{workspace}/sessions/{session}/mcp-refresh` queues the
   Codex app-server `config/mcpServer/reload` operation. The coordinator carries
   a pending generation across the next coding-agent start and enumerates
   `mcpServerStatus/list` before publishing a terminal snapshot. Clustered
   executions rematerialize the current settings-owned MCP section on the
   assigned worker first.
2. The workspace chat toolbar exposes **Restart agent for MCP changes**. It uses
   `POST /api/sessions/{session}/queue/mcp-restart`, the normal follow-up path,
   and a synthetic visible continuation. Idle sessions start immediately;
   running sessions require confirmation and transfer ownership through the
   existing queued-message finalizer. Warm Codex state is reaped first.

The UI deliberately switched from live refresh to restart because the latter is
the executor-neutral correctness boundary. This already meets the key product
requirement—no workspace recreation—but existing tests prove queue lifecycle,
not exact changed tool definitions at the fresh process boundary.

## Protocol facts

Vibe Kanban pins `@openai/codex@0.144.1`. Its app-server protocol includes:

- `config/mcpServer/reload`, returning an empty acknowledgement;
- thread-scoped `mcpServerStatus/list`, whose full detail returns server names,
  tools, resources, resource templates, and auth status.

The acknowledgement proves only that reload was accepted. Status listing is
the strongest public post-start inventory evidence, but it has no generation ID
and does not expose the complete model request envelope. Therefore the product
must not infer per-server restart or atomic preservation facts beyond what the
protocol reports.

## Failure hypothesis

The observed stale stdio tool list is consistent with consulting connector test
inventory or invoking the old live session without using the explicit toolbar
restart. Stdio servers are child processes whose advertised inventory is tied to
their connection. Re-reading a settings registry cannot replace the tool
definitions already injected into a model turn.

The implementation audit will still check for a narrower defect: whether an
idle “restart” can fork/resume a Codex thread in a way that reuses stale MCP
state, or whether current Codex startup reliably creates a fresh app-server and
reloads the native config before `thread/start`/`thread/fork`.

## Audit result

The restart boundary is correct. Every `spawn_inner` call invokes
`spawn_app_server`, which starts a new `codex app-server --strict-config`
process. Only after app-server initialization does the normal chat path call
`thread/start` or `thread/fork`, register the resolved thread, and call
`turn/start`. The MCP restart route also reaps a retained warm process before it
enters this normal follow-up path. A fork preserves conversation lineage inside
the newly started app-server; it does not reuse the previous app-server process
or its stdio MCP children.

The internal live-refresh chain also rematerializes current profile settings for
the assigned worker before queuing Codex reload. Its post-start confirmation was
weaker than the requested contract because `McpServerRefreshSnapshot` retained
only tool counts. Equal counts could not distinguish remove-plus-add or a
schema-only update. The implementation now carries sorted tool identifiers and
a stable SHA-256 of their input/output schemas from thread-scoped, full-detail
`mcpServerStatus/list` results. The coordinator replaces the complete evidence
vector for each successful generation.

The management UI does not label MCP assignments as plugin installations. The
reported external plugin-manager message is a separate domain. No Vibe Kanban
UI conflation was found. The integration guide was stale, however: it still
described the removed **Refresh MCP tools** toolbar instead of the shipped
**Restart agent for MCP changes** action, so the guide was corrected.

## Decisions

- Preserve the explicit restart as the primary UI contract.
- Do not automatically restart every active session when shared settings save.
- Test inventory replacement at the Codex protocol/fresh-process boundary with
  exact tool definitions, including schemas, using the smallest deterministic
  fake server/client seam available in the existing executor test harness.
- Keep the internal live-refresh API but correct any discovered false-success or
  stale-status behavior; do not expose it as stronger than the evidence permits.
- Treat plugin bundle installation and MCP native assignment as different
  domains. Correct misleading Vibe Kanban wording only where the product itself
  conflates them.
- Add no dependency unless the existing JSON-RPC test utilities cannot model
  the lifecycle deterministically.

## Alternatives rejected

- **Independent `mcp_test.rs` probe as success evidence:** it spawns its own MCP
  client and cannot prove the coding-agent runtime adopted the result.
- **Silent automatic process restart on settings save:** shared settings can
  affect several sessions and interrupt/queue work without explicit user intent.
- **Mutating tools during an in-flight model turn:** the model request schema is
  already fixed and partial replacement would violate atomic-generation rules.
- **Equating catalog or plugin presence with installation:** catalog suggestions,
  plugin bundles, and native MCP assignments have different sources of truth.
