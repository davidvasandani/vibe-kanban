# Contract: Workspace Affinity

## PATCH `/api/workspaces/{workspace_id}/affinity`

Request:

```json
{
  "requested_worker_node_id": "6de9c8b4-1a90-4b4e-84a5-f3ec46cb24bb",
  "restart_running": true,
  "operation_id": "4431a457-88f6-4b70-b8b4-946273d095bf"
}
```

- `requested_worker_node_id`: UUID for explicit affinity; null for automatic.
- `restart_running`: explicit authorization for managed stop/reassign/restart.
- `operation_id`: client-generated idempotency identity. Required when `restart_running` is true; optional for stopped reassignment. Repeating an ID with a different target is rejected.

Success or restart partial-success data:

```json
{
  "success": true,
  "data": {
    "placement": {
      "workspace_id": "...",
      "worker_node_id": "...",
      "placement_state": "ready",
      "placed_at": "2026-08-05T16:00:00Z",
      "placement_reason": "manual worker selection",
      "requested_worker_node_id": "...",
      "placement_constraints": null
    },
    "outcome": "restarted",
    "stopped_execution_id": "...",
    "started_execution": {},
    "message": null
  }
}
```

`outcome`:

- `updated`: stopped workspace affinity changed.
- `restarted`: running task stopped, affinity changed, and one continuation started.
- `restart_failed`: stop and affinity change are durable, but continuation failed; `message` is actionable and `started_execution` is null.

Conflicts (`409`):

- `confirmation_required`: exactly one coding agent is running and `restart_running` is false.
- `persistent_process_running`: dev server/background helper must be stopped first.
- `ambiguous_running_executions`: more than one coding agent is running.
- `migration_in_progress`: a different operation already owns the workspace.
- `stale_placement`: placement changed since the operation began; caller refetches.
- `stop_unconfirmed`: the old execution did not reach an evidenced terminal state; placement unchanged.

Bad request (`400`):

- unknown explicit worker,
- ineligible/offline/draining/unhealthy target,
- automatic scheduling has no eligible worker,
- operation ID reused with different request content.

## Workspace summary extension

Each `WorkspaceSummary` gains:

```json
{
  "affinity": {
    "kind": "worker",
    "placement_state": "ready",
    "worker_node_id": "...",
    "worker_hostname": "think3",
    "requested_worker_node_id": "...",
    "requested_worker_hostname": "think3"
  }
}
```

`kind` rules:

- `local`: placement state is local and no worker is assigned.
- `worker`: explicit requested worker is assigned.
- `automatic`: worker is scheduler-selected (`requested_worker_node_id` null) or automatic placement awaits assignment.
- `unassigned`: state cannot truthfully resolve to the other kinds.

## Managed continuation prompt

The exact localized-independent prompt is a source constant and communicates:

1. Vibe Kanban migrated this workspace to another execution server.
2. Inspect the current worktree/git state and prior conversation before acting.
3. Continue unfinished work without repeating completed work.
4. Preserve user changes and report any state that cannot be reconciled.
