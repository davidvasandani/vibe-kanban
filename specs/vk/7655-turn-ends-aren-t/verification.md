# Verification: Turn completion clears the running composer

Verified on 2026-09-02.

## Regression proof

Before implementation, the new
`signal_driven_turn_does_not_treat_live_child_as_turn_liveness` test failed: the
live Codex app-server child caused final-output reconciliation to re-arm
forever.

After implementation:

- `cargo test -p local-deployment final_output_reconciliation_tests --lib`:
  6 passed.
- `cargo test -p local-deployment --lib`: 40 passed.

The focused tests preserve both sides of the lifecycle contract: natural-exit
executors retain `running` while their owned child is alive, while
signal-driven executors reach bounded reconciliation because child liveness is
not turn liveness.

## Repository checks

- `pnpm install --frozen-lockfile`: passed.
- `pnpm run format`: passed.
- `git diff --check`: passed.
- `pnpm run check`: passed, including local-web, remote-web, web-core, UI, the
  primary Rust workspace, and the remote Rust workspace.

No frontend, API, schema, generated type, dependency, or homelab deployment
change was required.

## Cluster follow-up (2026-09-03)

The first release fixed local child-process liveness but the symptom persisted
for clustered executions. Runtime worker state confirmed the affected workspace
execution remained `running` until an external kill. The coordinator had a
second indefinite deferral: every current worker lease re-armed final-output
reconciliation, including leases for signal-driven Codex app servers.

The follow-up makes worker leases turn-liveness evidence only for natural-exit
executors. The focused suite now includes seven passing tests, including the
worker-lease predicate and exhaustive classification of the current
signal-driven and natural-exit executor variants.

## Worker terminal-signal follow-up (2026-09-03)

The coordinator fallback still could not arm in production because clustered
executor stdout reaches it as raw worker bytes, not normalized JSON patches.
The actual owning failure was in `crates/worker`: `run_job` discarded
`SpawnedChild.exit_signal` and polled only the OS child. Codex therefore emitted
its final answer and signalled completion while the worker waited forever for
the intentionally long-lived app-server process.

After retaining and consuming the executor signal:

- `cargo test -p worker executor_signal_maps_to_terminal_state_without_os_exit --lib`:
  1 passed.
- `cargo test -p worker --lib`: 63 passed.
- `cargo fmt --all`: passed.
- `git diff --check`: passed.

Independent review identified and the implementation now preserves a concurrent
`Cancelling` state when the executor completion channel closes, preventing a
user stop from being misclassified as `Failed`. The full 63-test worker suite
passed again after that correction.
