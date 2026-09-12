# Clarifications: Vibe Kanban Soft Restarts

**Spec**: `./spec.md`
**Status**: Resolved

## Decisions

### What is the stable owner in the deployed service?

The existing cluster worker. It already owns agent process groups, output journals, cancellation, approvals, previews, and PTYs independently of coordinator HTTP connections. Reusing it is safer than adding a second bootstrap protocol.

### Which restart becomes soft?

Coordinator application restarts are soft for worker-owned executions. Worker binary activation is deferred until that worker has closed admission and owns no active execution or terminal. The feature does not pretend a worker binary can replace itself while preserving its Rust-owned child handles.

### What happens to old workers without the drain protocol?

Automatic activation fails safe and reports that a one-time manual idle activation is required. Sending the new drain signal to an old binary could terminate it; inferring safety from a process snapshot has a check/dispatch race. Neither is acceptable.

### What should the browser do?

Keep the last initialized workspace snapshots rendered, reconnect with bounded jittered exponential backoff, and display an additive status after initial load. It does not clear route state or show a crash screen merely because the coordinator is temporarily unavailable.

## Remaining Questions

None for this feature. Standalone/local process supervision and PTY reattachment are explicit future scope.
