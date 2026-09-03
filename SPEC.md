# Technical Spec: Refresh Active Workspace MCP Inventories

**Task:** `vk/d71c-refresh-active-w`

## Problem

An active coding-agent session can outlive the MCP configuration and capability
inventory from which its callable tool registry was created. When a configured
stdio MCP is enabled, upgraded, or changes the result of `tools/list`, the
connector settings UI may display the new inventory while the running agent
continues to receive the old tool definitions. Added tools are therefore
uncallable, removed tools may remain advertised, and changed input schemas may
remain stale until the workspace or agent session is recreated.

The repository already contains an MCP-refresh path and a UI restart fallback.
This task must trace that path end to end, identify why the observed stale
stdio inventory can still occur, and close the gap without regressing remote
HTTP/SSE MCP configuration.

## Goal

Provide one supported refresh operation that re-materializes the effective MCP
configuration, obtains the current capability inventory, and makes additions,
removals, and input-schema changes visible to the active session on its next
turn. Where an executor cannot safely reload in place, present a clear,
one-click session refresh/restart that preserves the workspace and explains the
effect.

## Functional requirements

1. Refresh uses the latest effective connector configuration rather than the
   execution's original snapshot.
2. For supported active sessions, refresh reaches the executor runtime that
   owns the callable registry; updating connector metadata alone is not
   considered success.
3. A stdio MCP is reinitialized as necessary and `tools/list` is evaluated
   again. The next model turn observes exact tool additions, removals, and input
   schema changes.
4. Refresh is safe at turn boundaries. Concurrent refreshes and active tool
   calls produce deterministic, actionable states rather than partially
   replacing a registry.
5. Unsupported executors/transports fail explicitly and the UI offers the
   existing-session restart/refresh fallback without requiring workspace
   recreation.
6. The connector inventory, installed/enabled status, refresh response, and
   agent-visible registry derive from the same effective configuration or carry
   enough generation/status information to detect and explain staleness.
7. Failures are secret-safe and preserve the last known usable registry when
   possible.
8. Existing HTTP/SSE MCP behavior remains unchanged unless investigation shows
   it shares the same defect; at least one non-stdio regression test pins that
   boundary.

## Investigation and design constraints

- Compare connector probing and stored inventory with execution-time MCP config
  materialization and Codex app-server reload/list APIs.
- Verify whether the current `refresh_mcp_tools` API only refreshes inventory
  metadata, queues executor work, or actually changes the schema supplied on a
  subsequent model turn.
- Trace local and worker-owned executions separately so refresh is delivered to
  the process that owns the live session.
- Treat plugin installation state and connector enablement as distinct concepts
  only if the UI names that distinction clearly; contradictory status for the
  same effective connector is not acceptable.
- Keep all source changes within the Vibe Kanban repository. Change
  `homelab/modules/vibe-kanban-rebuild.nix` only if deployment wiring is required.

## Verification

Automated coverage must include:

- a stdio server adding a tool after session start;
- removal of an existing stdio tool;
- a changed input schema for an existing stdio tool;
- the next-turn application boundary, not only the refresh API response;
- failed refresh retaining or clearly invalidating the prior usable inventory;
- at least one HTTP or SSE regression case;
- UI behavior for supported refresh, queued/busy/error states, and explicit
  restart fallback;
- agreement between effective connector state and the status shown to users.

Run focused Rust and frontend tests, generated-type checks when shared types
change, formatting, and repository-wide checks proportionate to the final diff.

## Acceptance criteria

- An active workspace task can call a newly enabled or newly advertised stdio
  MCP tool after the supported refresh flow, without recreating the workspace.
- Tool removal and input-schema changes are reflected on the next turn.
- The refresh operation demonstrably re-runs capability discovery and updates
  the active runtime, or an explicit one-click restart performs that update with
  clear messaging where hot reload is unsupported.
- Connector inventory/status and the agent-visible callable registry agree after
  refresh.
- Add/remove/schema-change stdio tests and a non-stdio transport regression test
  pass.

## Out of scope

- Changes to services other than Vibe Kanban.
- General plugin marketplace redesign.
- Silent mutation of an in-flight model turn's tool schema.
