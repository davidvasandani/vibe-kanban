# Clarifications: Authoritative Execution Status Reconciliation

## Resolved Decisions

### Only `running` is active

The existing closed status domain is `running`, `completed`, `failed`, `killed`,
`interrupted`, and `indeterminate`. Only `running` is active and cancellable.
Every other value clears Stop. `indeterminate` means the coordinator cannot
prove the remote process's terminal outcome; it is still non-running and
non-cancellable in the composer rather than a reason to spin forever.

### Reconnect snapshots already exist, but their capture is racy

The execution-process session stream already sends `replace
/execution_processes` followed by `Ready` on every new WebSocket. The client
preserves the prior snapshot during reconnect and correctly applies a replace.
The server currently queries the database before it subscribes to the broadcast
channel. A process can become terminal between those steps: the snapshot says
running and the terminal broadcast has already passed before the receiver
exists. This explains why even a reconnect can retain stale running state.

The stream initialization contract must close that query/subscribe gap (or
otherwise replay/requery before declaring readiness), while retaining the
client's last-good rendering during transport outages.

### Shutdown recovery remains evidence-based

Existing shutdown/restart reconciliation must be verified as part of the fix.
Local non-persistent executions that cannot survive shutdown should become
`interrupted`; remote uncertainty may become `indeterminate`. Neither status is
active in the composer. A disconnect by itself is not reclassified as
`completed` or `killed`.

## Remaining Open Questions

None.
