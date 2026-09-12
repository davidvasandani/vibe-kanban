# Data Model: Stale Execution Recovery

## Existing durable entities

### ExecutionProcess

- `id`, `session_id`, `run_reason`
- `status`: `running | completed | failed | killed | interrupted | indeterminate`
- `started_at`, `completed_at`, `exit_code`, `pgid`

Only exact `running` is active. Final reconciliation writes one existing
terminal value; no new status is needed.

### ExecutionWorkerJob / WorkerNode

- worker assignment and dispatch state
- ordered worker event cursor/output completeness
- worker/job lease expiry and terminal evidence

These fields are positive remote authority. General metrics are excluded.

### Reconciliation registration

An in-memory quiet-window record owned by the same container that owns execution
handles:

```text
FinalOutputReconciliation {
  execution_id,
  observed_at,
  deadline = observed_at + 45 seconds,
  generation/cancellation token
}
```

Normal terminal completion removes/cancels the record. New log/worker events
reset `observed_at`. The deadline itself need not survive coordinator
replacement: startup reconciliation already handles durable `running` rows and
preserves WIP before classification, so persisting a second lifecycle clock
would create competing owners. No schema expansion is required.

## Client stream state

```text
JsonPatchStreamState<T> {
  data?: T,                    // may be allocated before authority
  readyForEndpoint?: endpoint, // authoritative snapshot received
  connected: boolean,
  consecutiveUnreadyFailures: number,
  lastClose?: { code, reason, wasClean }
}
```

Allocation does not imply Ready. Same-endpoint reconnect retains data only when
it came from a prior Ready.
