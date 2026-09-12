# Data Model: Coordinator Workspace Placement

No persistent schema changes are required.

## Request intent

| Input | Type | Meaning |
| --- | --- | --- |
| `run_on_coordinator` | boolean, defaults false when absent | Explicit coordinator-local intent |
| `requested_worker_node_id` | optional UUID | Explicit worker intent when present; automatic intent when absent and coordinator is false |

## Internal placement intent

| Variant | Result in cluster mode |
| --- | --- |
| Automatic | Scheduler selects an eligible worker; placement is reserved |
| Coordinator | Initial local placement is retained; no worker is reserved |
| Worker(UUID) | Scheduler validates/selects that worker; placement is reserved |

`run_on_coordinator = true` together with a worker UUID is invalid.
