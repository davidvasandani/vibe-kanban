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
