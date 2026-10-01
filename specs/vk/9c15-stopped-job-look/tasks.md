# Tasks: A worker restart never leaves a stopped job looking Running

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes. All code tasks share `crates/local-deployment/src/container.rs`, so
they are serial.

## Phase 1: Pure helpers
- [x] T001 Extract `terminal_summary_states(summary)` from
  `replay_gap_terminal_evidence` and make the latter delegate to it, in
  `crates/local-deployment/src/container.rs`
- [x] T002 Add `worker_journal_regressed(cursor, latest_available)` and
  `journal_regression_terminal_evidence(known, worker_node_id, execution_id,
  latest_available, summary)` in `crates/local-deployment/src/container.rs`
  (depends on T001)

## Phase 2: Tracker wiring
- [x] T003 In `track_worker_msgs_in_store`, after a successful `events` call,
  branch on `worker_journal_regressed`. Lookup error → back off and re-poll.
  Matching evidence → mark output incomplete, notice, set `terminal` and fall
  through to the existing terminal block. No evidence → incomplete, notice,
  `mark_remote_execution_indeterminate`, finalize, break.
  File: `crates/local-deployment/src/container.rs` (depends on T002)

## Phase 3: Validation
- [x] T004 Unit tests in `final_output_reconciliation_tests`: the regression
  predicate; the matcher for all four terminal states, identity/state/evidence
  and generation mismatches, and the incident shape; and unchanged replay-gap
  tests. File: `crates/local-deployment/src/container.rs` (depends on T002)
- [x] T005 Run `cargo test -p local-deployment`, `pnpm run backend:check`,
  clippy and `pnpm run format`; fix any findings (depends on T003, T004)

## Phase 4: Knowledge
- [x] T006 [P] Update `wiki/coordinator-restart-handoff.md` (journal
  regression section, rejected alternatives, contributed-by) and
  `wiki/INDEX.md` (depends on T005)
