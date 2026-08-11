# Research: Vibe Kanban Soft Restarts

## Finding: the bootstrap already exists

The production cluster already separates application and agent lifetimes:

- `crates/worker/src/execution.rs` owns agent process groups and idempotent dispatch by coordinator execution ID/digest.
- `crates/worker/src/journal.rs` retains monotonically sequenced output and explicit replay gaps.
- `crates/worker/src/server.rs` re-registers after coordinator disconnect/restart.
- `crates/services/src/services/cluster/reconcile.rs` reconciles worker inventory into coordinator SQLite before cleanup.
- `crates/services/src/services/container.rs` explicitly excludes worker-owned running rows from coordinator orphan cleanup.
- clustered preview and terminal traffic are worker-owned/proxied.

This is the JVM-style bootstrap concept expressed as a process boundary: agents remain attached to the worker while the coordinator application is replaceable.

## Root cause in deployment

`homelab/modules/vibe-kanban-rebuild.nix` preserves immutable releases, public health gating, and rollback for the coordinator, but worker distribution flips `current` and unconditionally runs `systemctl restart vibe-kanban-worker.service`. Because systemd owns the worker cgroup, that restart kills the worker and its agent descendants. The deployer was replacing the stable owner it needed to preserve.

## Decision: drain/defer worker activation

Coordinator restarts remain ordinary restarts; worker agents survive and replay/reconcile afterward. Worker binary updates do not attempt unsafe Rust handle transfer. They close new-work admission, wait for authoritative active execution to reach zero, then restart. Busy workers stay on their compatible immutable release and the existing timer retries later.

Admission close and ownership inspection must be one fenced sequence. Reading health then deciding is racy: a dispatch can land between check and restart. SIGUSR1 closes admission and health acknowledges it; only then is `drain_safe` meaningful. A marker persists the fence across candidate health/rollback.

Old workers cannot be signalled safely because SIGUSR1 may use its default terminating action. A cgroup process count is also only a snapshot with the same dispatch race. Therefore the automated path fails closed and asks for one manual idle activation to bootstrap the protocol.

## Browser root cause

`useJsonPatchWsStream` already retries unexpected closure and avoids surfacing an error after data exists, but `retryNonce` is an effect dependency and effect cleanup clears `dataRef`, `data`, and initialized state. Every retry therefore discards the last good screen before reconnecting—the white-page behavior.

Reset belongs to endpoint/enablement change, not same-endpoint transport retry. Keeping initialized data while `isConnected=false` lets the app render its last snapshot and show an additive status. Jitter avoids synchronized reconnect bursts from the many workspace/process streams.

## Alternatives rejected

- **New supervisor/gateway protocol**: duplicates the shipped worker ownership, replay, reconciliation, terminal, and preview machinery.
- **PID/pgid adoption for ordinary agents**: cannot preserve stdin, approvals, stateful log processing, or executor protocol channels.
- **Restart a busy worker and auto-resume**: compensation loses in-memory agent context and violates the explicit stable-owner goal.
- **Infer idle from metrics or one cgroup snapshot**: not authoritative and races new admission.
- **Hot-load Rust application code**: unsafe and unnecessary when the process boundary already exists.

## Dependencies

No new dependency is needed. The implementation uses existing Tokio signal/filesystem primitives, worker registries, health JSON, the Nix/systemd deployment, and React stream hooks.
