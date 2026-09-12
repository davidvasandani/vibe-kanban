# Implementation Plan: Refresh Active Workspace MCP Inventories

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical Context

- Backend: Rust 2024, Tokio, Axum, SQLx/SQLite, executor-native MCP configs, and
  Codex app-server JSON-RPC pinned to `@openai/codex@0.144.1`.
- Frontend: React/TypeScript in `packages/web-core`, using the shared queue API
  and workspace execution projections.
- Existing correctness path: same-session agent restart through
  `queue_mcp_restart`, `QueuedMessageService`, and normal `follow_up` launch.
- Existing optional live path: session-keyed `McpRefreshCoordinator`, local or
  assigned-worker refresh controls, Codex reload, and next-turn status listing.
- Constraints: no non-Vibe-Kanban service changes, no unproven protocol claims,
  no database migration expected, and no new dependency unless testability
  requires one.

## Architecture & Approach

### 1. Audit the fresh-process boundary

Trace `restartAgentForMcpChanges.ts`, `SessionChatBoxContainer.tsx`,
`routes/sessions/queue.rs`, `QueuedMessageService`, follow-up execution creation,
and Codex `spawn`/`thread_start` or `thread_fork`. Confirm that reaping warm state
and launching a new app-server causes the latest native MCP config to be read
before the next turn. Fix only a demonstrated reuse or stale-snapshot gap.

### 2. Make exact inventory replacement testable

Use existing Codex JSON-RPC/client test seams if possible. Model successive
full-detail server status/tool definitions for one stable server ID and assert:

- generation 1 → generation 2 adds a tool;
- generation 2 → generation 3 removes a tool;
- generation 3 changes the retained tool's complete input schema;
- each read is a complete replacement, including pagination where relevant.

If `McpRefreshResult` currently erases schemas, do not expand it merely for UI
display. Place the assertion at the nearest executor-owned protocol structure
that feeds the next turn, and separately ensure public counts/status do not
claim more than they know.

### 3. Pin the transport boundary

Add a shared MCP materialization/config regression proving a streamable-HTTP
definition remains assigned and unchanged across fresh process materialization.
Do not restart or probe remote transport independently unless runtime evidence
shows it is necessary.

### 4. Reconcile status wording

Audit Vibe Kanban's MCP management cards and refresh/restart action for any use
of “installed.” Ensure native assignment, enabled state, connectivity inventory,
and catalog suggestion are distinct. The external Codex plugin manager is not
an MCP assignment authority and is outside this UI contract.

### 5. Verification

Run `pnpm install --frozen-lockfile` before formatting. Execute focused executor,
service/server, and web-core tests; regenerate types only if public Rust types
change; run `pnpm run format`, relevant check/lint commands, and broader Rust
tests proportionate to the diff. Record exact results in `verification.md`.

## Data Model

See `./data-model.md`. No persisted schema change is planned.

## Contracts

See `./contracts/mcp-inventory-refresh.md`.

## Research Notes

See `./research.md`.

## Constitution Check

- Principles II and XVII require exact next-turn capability evidence and atomic
  replacement; counts or probe success alone do not pass.
- Principles VI and XII require reuse of the shipped restart/queue handoff.
- Principle IX requires all Codex claims to match the pinned app-server protocol.
- Principle XVIII requires refresh/routing to remain worker-affinity-bound.

No constitution deviation is planned.

## Risks & Dependencies

- Codex public status may be the strongest available proxy rather than the exact
  model request payload. Tests must label that limitation and avoid false claims.
- Full end-to-end process tests may be expensive; deterministic JSON-RPC fixtures
  must still cross the reload/start boundary rather than becoming another
  independent connector probe.
- Root SpecKit command templates currently name another task's owned path. This
  run preserves that record and writes only to this task directory.
