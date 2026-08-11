# Data Model: Worker Release Drain

No product database migration is required. The feature uses authoritative worker runtime state and one recovery-directory marker.

## AdmissionDrain

| Field | Source | Meaning |
| --- | --- | --- |
| `admission_draining` | worker atomic + persisted marker | New coding-agent execution admission is closed |
| `active_execution_count` | `ExecutionSupervisor` job states | Non-terminal worker-owned jobs |
| `drain_safe` | derived | Admission closed and execution count is zero |

Invariant: `drain_safe = admission_draining && active_execution_count == 0`.

## Drain marker

`<worker state dir>/release-admission-drain` is written before the worker acknowledges SIGUSR1 and removed before it acknowledges SIGUSR2. A candidate worker initializes admission from this marker, preventing a restart/health-gate window in which it could accept work that rollback would kill.

The marker is control state, not liveness evidence. Active counts continue to come from the owner registries.

## Existing execution state

`execution_worker_jobs` and the worker event journal remain unchanged. The coordinator already retains worker-owned `Running` rows during orphan cleanup and reconciles inventory/events when it restarts.
