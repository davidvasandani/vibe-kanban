# Data Model: Workspace Server Affinity and Migration

## Existing persisted entity: Workspace placement

Stored on `workspaces` and projected as `WorkspacePlacement`:

| Field | Meaning |
| --- | --- |
| `id` / `workspace_id` | Stable workspace identity. |
| `worker_node_id` | Worker currently assigned to execute this workspace; null for local placement. |
| `placement_state` | `local`, `reserved`, `provisioning`, `ready`, `failed`, or `cleaning`. |
| `placed_at` | Time current assignment was made. |
| `placement_reason` | Operator/scheduler-readable reason for the assignment. |
| `requested_worker_node_id` | Explicit affinity; null means automatic. |
| `placement_constraints` | Scheduler constraints, retained across reassignment where applicable. |

No schema change is required for affinity itself.

## Existing persisted entity: Worker node

`WorkerNode` supplies ID, hostname, status, mount health, lease, capabilities, and resource snapshot. New affinity selection uses the existing scheduler eligibility predicate; UI eligibility is advisory only.

## Existing persisted entity: Execution process

The running `CodingAgent` execution supplies:

- exact process ID to stop,
- owning session ID,
- serialized executor action containing the authoritative `ExecutorConfig`,
- durable lifecycle status/evidence.

The continuation is a new execution process in the same session.

## New API projection: WorkspaceAffinitySummary

Non-persisted fields included in each bulk workspace summary:

- `kind`: `local | automatic | worker | unassigned`
- `placement_state`
- `worker_node_id`
- `worker_hostname`
- `requested_worker_node_id`
- `requested_worker_hostname`

The backend supplies facts; localization and compact label formatting stay client-side.

## New API outcome: WorkspaceAffinityUpdateResponse

- `placement`: resulting `WorkspacePlacement`
- `outcome`: `updated | restarted | restart_failed`
- `stopped_execution_id`: optional
- `started_execution`: optional new `ExecutionProcess`
- `message`: optional actionable partial-failure detail

## Optional durable entity: affinity migration operation

Only add if existing execution idempotency cannot cover response-loss retries:

- `id`: operation UUID/idempotency key
- `workspace_id`
- requested worker ID (nullable)
- phase: `claimed | stopped | placed | restarted | restart_failed`
- stopped/new execution IDs
- timestamps/error detail

Unique `(workspace_id, active phase)` prevents concurrent migration; unique operation ID makes retries return the recorded result. Do not add this table if a smaller existing durable claim provides equivalent guarantees.

## State transitions

Stopped path:

`current placement → validate/select → compare-and-set placement → updated`

Running path:

`running → confirmation required` or, when confirmed:
`running → stop evidenced → placement updated → continuation created → restarted`

Partial terminal path:

`stop evidenced → placement updated → continuation creation failed → restart_failed`

Forbidden transitions:

- placement update before stop evidence,
- continuation before placement readiness,
- migration while persistent processes run,
- continuation when more than one coding-agent execution is running.
