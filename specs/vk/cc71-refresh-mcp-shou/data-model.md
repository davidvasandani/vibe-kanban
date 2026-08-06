# Data Model: Refresh Active Remote MCP Snapshots

No persistent schema change is required.

## Existing persistent relationships

- `Session` belongs to one `Workspace`.
- `ExecutionProcess` identifies the latest Codex execution/profile for a
  session.
- `WorkspacePlacement` identifies the assigned worker for a remote workspace.
- `ExecutionWorkerJob` binds an execution to the worker that accepted it.

## Ephemeral worker state

Each live `WorkerJob` gains optional MCP refresh state:

- execution-scoped Codex home/config path;
- resolved executor/profile adapter used for native MCP section replacement;
- retained live `McpRefreshHandle` once the Codex app-server bootstraps;
- a per-execution refresh claim/lock;
- current materialized snapshot generation or digest for diagnostics/tests,
  excluding secret-bearing values.

The state exists only for the lifetime of the live job. Recovery without a live
control cannot assert refresh support.

## Protocol values

`McpConfigSnapshot` remains the bounded settings snapshot:

- `executor`: stable base executor identifier;
- `servers`: native-shape map keyed by configured server identifier.

A worker refresh outcome carries only status/category metadata, never the map or
its environment/argument values.
