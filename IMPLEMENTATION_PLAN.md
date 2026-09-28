# Implementation plan — vk/80c1-tasks-should-sur

See `SPEC.md` for goals and acceptance criteria (AC1–AC7).

## Step 1: Raw-log writer that can skip seeded history

`crates/services/src/services/execution_process.rs`

- Add a `skip_raw_history: usize` parameter to
  `spawn_stream_raw_logs_to_storage`. The first N `Stdout`/`Stderr` messages
  are consumed without writing; `SessionId` and `MessageId` still update the
  DB. Existing callers pass `0`.
- Unit-test the skip count with a MsgStore seeded with 2 lines plus 1 new line,
  asserting the file gains exactly one line (AC4).

## Step 2: One shared log pipeline

`crates/services/src/services/container.rs`

- Extract the post-dispatch block of `start_execution` into a trait-provided
  `start_execution_log_pipeline(&self, workspace, process, action, run_reason,
  skip_raw_history)`. The block covers the normalizer, the raw-log writer
  (when the run does not write its own raw log), and the pipeline-stage
  tracker for coding agents.
- `start_execution` calls it with `0`.
- Add a trait method `reattach_worker_executions(&self)` with a default no-op.

## Step 3: Tracker starts from a cursor with a pre-seeded store

`crates/local-deployment/src/container.rs`

- `track_worker_msgs_in_store(process, worker, resume: Option<u64>)`. The
  cursor starts at `resume.unwrap_or(0)`. When resuming, the caller has already
  inserted the seeded store, so the tracker reuses it rather than replacing it.
- Replay-gap branch: when the inventory shows the same job identity and it is
  non-terminal, mark the output incomplete, push a notice, set
  `cursor = earliest_available - 1`, and `continue`. Pure predicate
  `replay_gap_live_job(known, worker, exec, summary) -> bool`, unit-tested (AC6).

## Step 4: Shutdown handoff

`LocalContainerService::kill_all_running_processes`

- Before the persistent check, if `ExecutionWorkerJob::find_by_execution_id`
  returns a job with a non-terminal `dispatch_state`, abort the exit-monitor
  (tracker) handle, log, record the handoff and `continue`. Do not call
  `stop_execution` and do not commit WIP.
- If any handoff happened, sleep `WORKER_HANDOFF_FLUSH_GRACE` (250 ms).
- Pure predicate `should_hand_off_worker_job(job) -> bool`, unit-tested (AC1).

## Step 5: Re-attach on boot

`LocalContainerService::reattach_worker_executions`

- For each row from `ExecutionProcess::find_running` with a non-terminal
  worker job: load context; `load_raw_log_messages`; build the MsgStore, push
  history and insert it into `msg_stores`; call `track_worker_msgs_in_store(..,
  Some(job.last_event_sequence))`; call `start_execution_log_pipeline(..,
  seeded_raw_count)`. The order matters: the tracker must not push before the
  writer subscribes. Both are spawned from the same task before any `.await`
  yields to the tracker's first poll. Because the writer skips by count rather
  than by subscription timing, ordering is not load-bearing.
- Wire it into `crates/server/src/main.rs` and
  `crates/server/src/startup.rs` right after `cleanup_orphan_executions`.

## Step 6: Reconciler defers running rows

`crates/services/src/services/cluster/reconcile.rs`

- In `apply_worker_evidence`, for a terminal summary whose process row is
  `Running`, call `observe_worker_sequence` only, increment
  `report.jobs_deferred` and return.
- Update the existing tests. Add a test that a terminal job with a `Running`
  row is left for the tracker (AC3).
- Log `jobs_deferred` in `local-deployment/src/lib.rs`.

## Step 7: Verify

- `cargo test -p services -p local-deployment -p server`, then
  `cargo test --workspace`.
- `pnpm run backend:check`, `pnpm run lint` (clippy), and `pnpm run format`.
- Types: no ts-rs change is expected; `ReconciliationReport` is not exported.

## Step 8: Docs and knowledge

- Wiki: new page `wiki/coordinator-restart-handoff.md`, plus updates to
  `agent-process-lifecycle.md` "What already survives restarts" and to the
  index.
- User docs: `docs/` troubleshooting or self-hosting note, if a page covers
  restarts.
