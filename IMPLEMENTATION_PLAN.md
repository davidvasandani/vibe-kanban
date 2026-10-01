# Implementation plan: vk/9c15-stopped-job-look

See `SPEC.md` for the root cause. Every change is in
`crates/local-deployment/src/container.rs`.

1. **Share the terminal state mapping.** Extract the `(JobState,
   TerminalState) → (dispatch state, process status)` match from
   `replay_gap_terminal_evidence` into `terminal_summary_states(summary)`.
   Replay-gap behavior is unchanged.
2. **Pure regression predicate.** Add
   `worker_journal_regressed(cursor, latest_available) -> bool`, which is
   `latest_available < cursor`. Document why it is unambiguous: every cursor
   value came from this worker's own journal.
3. **Pure evidence matcher.** Add `journal_regression_terminal_evidence(
   known, worker_node_id, execution_id, latest_available, summary)`. It
   requires an exact identity match (as in replay-gap recovery) and
   `summary.last_sequence == latest_available` (same journal generation),
   then returns the mapped terminal tuple.
4. **Tracker branch.** In `track_worker_msgs_in_store`, right after a
   successful `events` call, when `worker_journal_regressed(cursor,
   batch.latest_available)`:
   - look up the DB job and the worker inventory;
   - lookup error → warn, back off, `continue 'poll` (never infer);
   - matching evidence → `mark_output_incomplete`, push a stderr notice,
     set `terminal` and fall into the normal terminal handling (no new
     finalization path);
   - no matching evidence → warn, `mark_remote_execution_indeterminate`,
     `finalize_remote_execution`, `finish_msg_store`, `break`.
5. **Tests** in `final_output_reconciliation_tests`:
   - predicate: equal / ahead / behind / zero;
   - matcher: all four terminal states map; identity changes, missing
     terminal, non-terminal state, contradictory evidence and a
     generation mismatch return `None`; the incident shape (known cursor
     2549 vs recovered `last_sequence` 6) recovers `Interrupted`.
6. **Verify**: `cargo test -p local-deployment`, `pnpm run backend:check`,
   clippy, `pnpm run format`.
7. **Docs/KB**: update `wiki/coordinator-restart-handoff.md` with the
   journal-regression rule and the rejected "persist more often" alternative.
