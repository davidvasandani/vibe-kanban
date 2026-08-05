# Technical Specification: Workspace Server Affinity and Migration

**Task:** `vk/9a64-vk-workspace-aff`  
**Scope:** Vibe Kanban service only  
**Status:** Draft for implementation

## Objective

Make a workspace's execution-server affinity visible wherever an operator chooses or monitors a workspace, and allow the operator to change that affinity safely. A stopped workspace can be reassigned directly. A running workspace requires an explicit confirmation, after which Vibe Kanban stops the active task, changes affinity, and restarts the work through a Vibe Kanban-managed continuation prompt.

## Existing System

- Workspace placement is persisted through `WorkspacePlacement`, including the assigned `worker_node_id`, the requested `requested_worker_node_id`, and placement state.
- Workspace creation already exposes a **Run on** selector with automatic placement and eligible worker nodes.
- The frontend can fetch placement through `workspacesApi.getPlacement()` and workers through `workerNodesApi.list()`.
- Carousel workspace columns already render a placement label, but the primary workspace list drawer and workspace right drawer do not expose affinity.
- Worker eligibility is currently represented by online status plus healthy mounts; the coordinator/local placement is represented separately from worker rows.

## Terminology

- **Current server:** the coordinator/local host or worker node currently holding the workspace placement.
- **Affinity:** the operator's requested server choice. `null` means automatic placement; a worker UUID means that worker was explicitly requested.
- **Migration:** stopping active execution, updating affinity/placement, and starting a managed continuation on the new server.
- **Stopped workspace:** a workspace with no running execution process. Dev-server-only activity does not silently authorize migration and must be handled by the backend's workspace stop semantics.

## Functional Requirements

### 1. Left workspace drawer

Each workspace row in the primary left drawer MUST show compact server-affinity information without obscuring the workspace name or existing status/diff metadata.

- Show the resolved hostname for an assigned worker.
- Show the coordinator/local hostname or a clear `Local` label for local placement.
- Show `Automatic` or `Unassigned` when no concrete server has been assigned, choosing the label that accurately reflects requested affinity and placement state.
- Use the existing placement and worker query cache so rendering a list does not produce an avoidable request per workspace.
- Keep the information readable in running, attention, idle, and archived groupings.

### 2. Right workspace drawer

Add a **Server Affinity** accordion alongside the existing workspace panels.

When collapsed, its summary MUST communicate the current resolved server. When expanded, it MUST show:

- current placement/server,
- a **Run on** selector matching workspace creation semantics,
- `Automatic placement`, and
- known workers, with offline, draining, or unhealthy workers visibly unavailable for new selection unless they are the workspace's current selection (which remains identifiable).

The control MUST expose loading, mutation-in-progress, success, and actionable failure states and prevent duplicate submissions.

### 3. Reassigning a stopped workspace

Selecting a different affinity for a stopped workspace MUST call one backend operation that validates and persists the new affinity.

- `Automatic placement` stores no requested worker.
- A concrete selection stores that worker as requested affinity.
- The backend validates that a newly selected worker exists and is schedulable.
- The backend updates placement atomically and returns the resulting placement.
- Re-selecting the effective current affinity is a no-op.
- Placement, workspace-list, and worker-related query data are invalidated or updated immediately after success.

### 4. Migrating a running workspace

Changing affinity while any execution for the workspace is running MUST NOT happen immediately. The UI MUST display a confirmation dialog that clearly says Vibe Kanban will:

1. stop the currently running task,
2. move the workspace affinity to the selected server, and
3. start the task again with a Vibe Kanban-managed continuation prompt.

Cancel leaves all state unchanged. Confirm invokes a single backend migration workflow; the client must not coordinate a fragile sequence of independent stop/update/start requests.

The backend workflow MUST:

- serialize concurrent affinity changes for the workspace,
- re-check running state and target eligibility,
- stop active execution using existing stop/cancellation behavior and wait until it is no longer running,
- persist the requested affinity and make the workspace eligible for placement on the target,
- start one follow-up execution in the most recent applicable session using a predefined, version-controlled Vibe Kanban prompt,
- preserve the session's executor/profile configuration unless normal follow-up defaults require otherwise,
- return enough result data for the client to refresh placement and execution state, and
- report partial failure precisely (for example, stopped and reassigned but restart failed) without pretending the migration completed.

Managed prompt intent: continue the existing task after Vibe Kanban moved it to another execution server; first inspect the current workspace, git state, and prior conversation, then resume unfinished work without repeating completed work. This prompt is product-owned and is not editable in the confirmation dialog.

### 5. API contract

Add an authenticated workspace-scoped mutation, conceptually:

`PATCH /api/workspaces/{workspace_id}/affinity`

Request:

```json
{
  "requested_worker_node_id": "uuid-or-null",
  "restart_running": false
}
```

Behavior:

- With no running execution, `restart_running` may be false and the affinity is updated.
- With a running execution, false returns a conflict that tells the client confirmation/restart is required.
- With a running execution and true, the server performs the managed stop/migrate/restart workflow.

Response SHOULD include the resulting `WorkspacePlacement`, whether a running task was restarted, the new execution identifier when applicable, and a migration outcome suitable for precise error reporting. Exact naming will be finalized during SpecKit planning.

### 6. Consistency and failure handling

- Existing execution and placement authorization rules remain in force.
- Unknown/ineligible targets are rejected server-side even if the UI is stale.
- A failed stop leaves affinity unchanged.
- A failed placement update does not start a new execution.
- A failed restart leaves the workspace stopped on the requested affinity and returns a distinct recoverable outcome.
- Multiple confirmation clicks cannot create multiple restarts.
- The left drawer, right drawer, carousel, terminal/editor routing, and subsequent executions converge on the returned placement.

## Non-Functional Requirements

- Follow the local web design tokens and container/view separation rules.
- Reuse existing accordion, select, confirmation-dialog, query, stop, and follow-up primitives where practical.
- Do not manually edit generated TypeScript types; update Rust declarations and regenerate them.
- Add focused backend tests for validation, stopped reassignment, running conflict, successful migration, and restart failure behavior.
- Add frontend tests for affinity labels, eligible/disabled choices, stopped updates, running confirmation/cancel, and mutation outcomes.
- Avoid N+1 polling from workspace rows; placement summaries should be fetched in bulk or carried by the existing workspace summary contract.
- All operator-facing strings are localized.

## Out of Scope

- Moving repositories, data, or services outside the Vibe Kanban workspace execution model.
- Editing other homelab service modules.
- Worker registration, draining controls, scheduler scoring, or capacity-management redesign.
- Automatically evacuating all workspaces from a draining/offline worker.
- Migrating dev servers independently of their workspace.

## Acceptance Criteria

1. Every workspace shown in the left drawer has an accurate compact server-affinity label.
2. The right drawer contains a Server Affinity accordion with current placement and a creation-style Run on selector.
3. Changing a stopped workspace to automatic or an eligible worker succeeds without a restart prompt and updates both drawers.
4. Changing a running workspace always requires explicit confirmation.
5. Canceling confirmation performs no stop, placement update, or restart.
6. Confirming performs one server-managed stop/reassign/restart operation and creates exactly one managed continuation execution.
7. Ineligible or stale target selections fail safely with a useful message.
8. Partial failures are accurately surfaced and never cause duplicate executions.
9. Relevant backend/frontend tests pass, generated types are current, and formatting/lint/type checks succeed.

## Open Questions for SpecKit Clarification

- Whether `Automatic placement` should immediately reschedule a stopped but already placed workspace, or only clear requested affinity for its next execution.
- Whether local/coordinator placement should be selectable explicitly or represented only through automatic placement.
- Which existing session and executor configuration is authoritative when restarting a running workspace with multiple sessions.
- Whether workspace-wide stop includes active dev servers for this migration workflow.
