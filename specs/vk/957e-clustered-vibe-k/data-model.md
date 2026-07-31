# Data Model: Clustered Vibe Kanban

## Worker node

`worker_nodes`

| Field | Contract |
| --- | --- |
| `id` | Stable UUID/text primary key |
| `hostname` | Last advertised hostname |
| `status` | `online`, `offline`, or `draining` |
| `worker_version` | Worker build/protocol compatibility input |
| `vibe_version` | Product version |
| `capabilities` | Validated JSON executor profiles/features |
| `resource_snapshot` | Validated JSON CPU/load/memory/execution metrics |
| `labels` | Validated JSON string map |
| `mount_status` | `healthy` or a specific unhealthy class |
| `mount_message` | Secret-free operator diagnostic |
| `last_heartbeat_at` | Coordinator receipt time |
| `lease_expires_at` | Scheduling/reconciliation boundary |
| `created_at`, `updated_at` | Audit timestamps |

Only the coordinator writes this table. A draining worker may heartbeat and
finish existing jobs but receives no new automatic placement.

## Workspace placement

Add to `workspaces`:

| Field | Contract |
| --- | --- |
| `worker_node_id` | Nullable for legacy/local workspaces; FK to worker |
| `placement_state` | `local`, `reserved`, `provisioning`, `ready`, `failed` |
| `placed_at` | Reservation timestamp |
| `placement_reason` | Bounded operator-visible reason |
| `requested_worker_node_id` | Optional manual request/audit |
| `placement_constraints` | Optional validated label constraints |

Transitions are monotonic for one workspace:
`reserved -> provisioning -> ready|failed`. `local` remains the default when
clustering is disabled. `worker_node_id` is immutable after reservation.

## Execution ownership

`execution_worker_jobs`

| Field | Contract |
| --- | --- |
| `execution_process_id` | Primary/FK to execution process |
| `worker_node_id` | Assigned worker |
| `worker_job_id` | Stable worker-side identity |
| `request_digest` | Digest of immutable dispatch payload |
| `dispatch_state` | `pending`, `accepted`, `running`, terminal/indeterminate |
| `last_event_sequence` | Last coordinator-persisted event |
| `worker_last_sequence` | Last worker-advertised sequence |
| `lease_expires_at` | Last worker evidence window |
| `output_complete` | False after unrecoverable replay gap |
| `terminal_evidence` | Bounded JSON status/exit/signal/timestamps |
| timestamps | Dispatch/accept/update/complete audit |

`execution_processes.pgid` remains local-only and null for remote jobs.
Indeterminate is added to the public execution status or represented by an
explicit ownership state without converting it to `failed`, `killed`, or
`completed`.

## Repository administration lock

`repository_admin_locks`

| Field | Contract |
| --- | --- |
| `repo_id` | Primary/FK |
| `generation` | Monotonically increasing fencing token |
| `operation_id` | Current coordinator operation |
| `acquired_at`, `lease_expires_at` | Stale-owner detection |

The single coordinator acquires a new generation in a SQLite transaction, then
holds the in-process per-repository lock through the Git operation. Logs include
the generation and operation ID.

## Preview target

Persist or retain in supervised job metadata:
`workspace_id`, `worker_node_id`, `worker_job_id`, `port`, and `generation`.
Proxy requests use all fields so later port reuse cannot satisfy an old target.

## Invariants

- A remote-ready workspace has exactly one worker and a canonical shared path.
- An execution worker must equal its workspace worker.
- One execution ID maps to one request digest and at most one process.
- Terminal execution state needs worker evidence or explicit user/coordinator
  interruption semantics.
- Cleanup cannot proceed while matching jobs are active or evidence is stale.
