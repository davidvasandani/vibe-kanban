# Implementation Plan: Turn completion clears the running composer

**Spec**: `./spec.md`
**Status**: Ready

## Technical Context

The owning code is Rust/Tokio in
`crates/local-deployment/src/container.rs`. The React composer already derives
Stop from streamed `ExecutionProcess.status === running`; its closed-status
truth table and snapshot replacement behavior are correct. Codex supplies an
`ExecutorExitSignal` separately from its long-lived app-server child.

## Architecture & Approach

1. Extend `wait_for_unfinalized_output` with the existing lifecycle fact that
   determines whether a live child proves turn liveness.
2. In `spawn_exit_monitor`, capture `exit_signal.is_some()` before consuming the
   receiver into its future and pass the corresponding process-liveness policy
   to reconciliation.
3. Preserve natural-exit behavior: only those executions defer while their
   child is alive.
4. Preserve signal-driven normal completion, but allow missing terminal signal
   plus quiet final output to reach the existing timeout, reap, and
   `indeterminate` persistence path.
5. Replace/expand the paused-time unit coverage to assert both lifecycle rows
   in `contracts/execution-final-output-reconciliation.md`.

No frontend, API, database, generated-type, or Nix module change is planned.

## Data Model

See `./data-model.md`. No migration is required.

## Contracts

See `./contracts/execution-final-output-reconciliation.md`.

## Research Notes

See `./research.md`. No new dependency is required.

## Verification

- Focused paused-time tests in `local-deployment` for signal-driven versus
  natural-exit live children.
- Existing local final-output reconciliation and relevant container tests.
- `cargo fmt --check` / repository `pnpm run format`.
- `cargo test -p local-deployment --lib`.
- `pnpm run check` if feasible after locked dependency setup, because the
  shared repository completion checklist requires it even though no TypeScript
  file changes.

## Constitution Check

- **II — Test the contract:** focused truth-table regression is required before
  implementation is complete.
- **III / VI — Small change, reuse shipped machinery:** the fix qualifies the
  existing bounded reconciliation helper using `SpawnedChild.exit_signal`; it
  adds no parallel status channel.
- **XV — Destructive operations fail safe:** reaping is limited to the exact
  container-owned process after bounded final output and applies only where
  process liveness is not turn liveness. Existing preservation/finalization
  remains intact.
- **XVIII / XXX — Evidence-backed, authoritative execution UI:** the database
  terminal status remains authoritative and the UI clears only after that
  transition.

No constitution deviation is required.

## Risks & Dependencies

- The key risk is misclassifying a signal-driven executor that legitimately
  emits final assistant output and continues work silently for longer than the
  bound. Meaningful normalized activity resets the observation, and the
  executor's explicit signal is the intended terminal authority; the fallback
  records uncertainty (`indeterminate`), never success.
- Existing tests may depend on the over-broad live-child behavior and must be
  updated to state the lifecycle assumption explicitly.
