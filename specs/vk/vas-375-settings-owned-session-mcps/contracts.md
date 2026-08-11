# Contracts: Settings-Owned MCP Execution Snapshot

## Coordinator Dispatch Contract

- Resolve the selected profile and its native coding-agent configuration.
- When supported, include one `McpConfigSnapshot` with the exact dispatched
  executor identifier and complete native server map.
- Treat snapshot read/adaptation failure as dispatch failure; do not silently
  fall back to worker or repository MCP definitions.
- Do not log the snapshot or its values.

## Worker Preparation Contract

- Reject snapshot/executor mismatch before child spawn.
- Resolve the agent's native config under the worker home and reject paths that
  escape it.
- Create one execution-scoped overlay and atomically materialize the map through
  the existing adapter.
- Preserve unrelated vendor settings and home assets.
- Return child-only environment overrides; never mutate process-global env.
- Remove the scoped tree when the prepared state is dropped.

## Refresh Contract

- Refresh remains available only for an active Codex execution.
- Replace only the MCP table in the prepared target file.
- Confirm adoption through the existing Codex protocol before reporting success.
- Preserve last known-good runtime state on failure and never expose secrets.

## Deployment Contract

- `homelab/.mcp.json` contains no Vibe Kanban MCP definition.
- Nix may provide runtime packages, routes, and executor prerequisite secrets but
  does not seed native MCP tables.
