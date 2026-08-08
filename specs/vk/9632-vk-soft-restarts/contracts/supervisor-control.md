# Contract: Worker Release Drain

## Signals

- `SIGUSR1`: persist the release-drain marker, then atomically close new coding-agent execution admission.
- `SIGUSR2`: remove the marker, then atomically reopen admission.
- `SIGTERM`/Ctrl-C: retain ordinary hard worker shutdown behavior.

The deployer waits for health acknowledgement rather than assuming signal delivery changed state.

## Health response

Unauthenticated `GET /health` remains a non-secret liveness endpoint and adds:

```json
{
  "status": "ok",
  "worker_node_id": "uuid",
  "active_execution_count": 1,
  "admission_draining": true,
  "drain_safe": false
}
```

These counts come directly from owner registries. Metrics are not used for lifecycle decisions.

## Admission behavior

- Repeating an already accepted execution ID and identical digest remains idempotently successful during drain.
- A new execution dispatch during drain returns `503 Service Unavailable`.
- New terminal creation during drain returns `503 Service Unavailable`.
- Events, acknowledgements, input, resize, cancellation, and explicit close for existing work remain available.

## Deployment behavior

The distributor requires the health response to contain `admission_draining`. It signals drain, waits for acknowledgement, and reads `drain_safe`. False/unknown defers without changing `current`; true permits immutable release delivery, symlink flip, restart, and health gate. Admission resumes only after success. Rollback restores the previous symlink, restarts, and resumes admission.
