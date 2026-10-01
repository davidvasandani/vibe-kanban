# Implementation Plan: A worker restart never leaves a stopped job looking Running

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context

- Rust coordinator, `crates/local-deployment/src/container.rs`. The worker
  event tracker `track_worker_msgs_in_store` polls
  `WorkerClient::events(worker, execution, after = cursor)` and gets an
  `EventBatch` (`cluster_protocol`) with `events`, `earliest_available`,
  `latest_available` and `replay_gap`.
- Worker: `crates/worker/src/execution.rs` (`with_recovery_and_drain`) and
  `crates/worker/src/journal.rs` (`EventJournal::recover`, `replay_after`).
  The worker is left unchanged. Its recovery is correct by its own rules: it
  numbers the terminal event after the last sequence it persisted.
- SQLite via `db::models::execution_worker_job` (`mark_output_incomplete`,
  `update_state`, `acknowledge_sequence`, which is monotonic) and
  `db::models::execution_process`.

## Architecture & Approach

One new branch in the tracker loop, built from pure helpers so the decision
logic is unit-testable:

1. `terminal_summary_states(summary)` is the shared `(JobState,
   TerminalState)` → `(ExecutionWorkerDispatchState,
   ExecutionProcessStatus, TerminalEvidence)` mapping, extracted from
   `replay_gap_terminal_evidence`. Replay-gap recovery keeps its own
   sequence rules and only delegates the mapping.
2. `worker_journal_regressed(cursor, latest_available)` returns
   `latest_available < cursor` (FR-1).
3. `journal_regression_terminal_evidence(known, worker_node_id,
   execution_id, latest_available, summary)` checks for an exact identity
   match and `summary.last_sequence == latest_available` (current stream,
   per the Clarifications), then maps the record (FR-2).
4. Tracker branch, placed right after a successful `events` call, before any
   event processing:
   - DB row plus `client.inventory`. A lookup error means warn, back off and
     `continue 'poll` (FR-5, never guess).
   - Matching evidence: `mark_output_incomplete`, a single stderr notice
     (FR-4), then set `terminal` and fall through to the **existing** terminal
     block. That block handles `update_state`, `was_stopped` precedence,
     `update_completion_with_retry`, both acknowledgements,
     `finalize_remote_execution` and `finish_msg_store` (FR-3).
   - No matching evidence: warn, mark output incomplete, push the notice,
     `mark_remote_execution_indeterminate`, `finalize_remote_execution`,
     `finish_msg_store`, `break` (FR-5). This is the same sequence the
     existing unreachable-after-final-output branch uses.
5. FR-6 needs no extra wiring. Boot re-attach (`reattach_worker_executions`)
   and live dispatch both run this one tracker, and `ExecutionReconciler`
   already defers terminal summaries under `Running` rows to it.

## Data Model

No schema change; see `./data-model.md`.

## Contracts

The worker HTTP protocol is unchanged. `./contracts/tracker-regression.md`
documents the coordinator-side invariant the new branch relies on.

## Research Notes

See `./research.md`.

## Constitution Check

- **XVIII** (evidence-backed, as refined in 0.42.1): the regression is
  resolved only from exact-identity terminal evidence, or classified
  indeterminate. A disconnect or lookup error never infers an outcome.
- **XXX** (UI from authoritative state): the backend row becomes terminal,
  and the existing stream snapshot clears Stop. No frontend change.
- **III / VI** (small, reuse): no new finalization path. The existing
  terminal block and `mark_remote_execution_indeterminate` are reused, and
  the state mapping is shared, not duplicated.
- **XII** (one authoritative owner): the tracker stays the only finalizer
  for worker rows, as `coordinator-restart-handoff` requires.
- **II** (test the contract): unit tests for both pure helpers.

No deviations.

## Risks & Dependencies

- **False positive on a healthy job.** This would need `latest_available <
  cursor` while the worker still holds the stream the cursor came from. Every
  cursor value is a sequence that worker served, and acknowledgement never
  moves the cursor, so a healthy journal cannot report less. Resume-from at
  boot is the DB `last_event_sequence`, also a served sequence.
- **Output lost after the cursor.** This is inherent to a worker crash, and
  the incomplete-output flag and notice surface it honestly.
- **Repeated inventory calls.** A regression is resolved on the first
  successful check, and only lookup failures loop, with the existing backoff.
