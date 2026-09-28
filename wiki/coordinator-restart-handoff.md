# Coordinator Restart Handoff: Worker Work Survives a VK Deploy

This page explains how a coordinator restart (a deploy, `systemctl restart`, or
a crash) leaves worker-owned executions running and picks them back up. It also
covers the traps for anyone who changes shutdown, boot reconciliation or the
worker event tracker.

## The bug was self-inflicted

Cluster workers are already a supervisor/runner split: the agent process's
parent is `vibe-kanban-worker`, not the coordinator. The deferred Tier-3
designs (see [[agent-process-lifecycle]]) were not needed for this case. The
coordinator was undoing the split itself, at two points:

1. **Shutdown cancelled remote work.** `kill_all_running_processes` sent every
   non-persistent row through `stop_execution(.., Interrupted)`. For a worker
   row that is a `CancellationRequest`. It then ran `commit_interrupted_wip`
   against a worktree the agent was still writing.
2. **Boot never re-attached.** `track_worker_msgs_in_store` was started only by
   `dispatch_execution`. Persistent worker rows (pollers, helpers, dev
   servers) did survive shutdown, but they came back as `Running` rows with
   nothing following them. No output arrived and no terminal state landed.

The fix is the smallest change that stops undoing the split: **hand off on
shutdown, and re-attach on boot**. The worker drain (homelab
`vibe-kanban-worker-distribute`: SIGUSR1, then `drain_safe`, then SIGUSR2)
already makes worker replacement wait for idle, pollers included. So "reload
only when idle" needed no change.

## Handoff: what shutdown must not do

`should_hand_off_worker_job` is true for **every** `Running` row that has a
worker job, whatever its dispatch state. Terminal states are included: a
crash between the tracker's job update and its row update leaves a terminal
job under a `Running` row. Terminal events are never acknowledged before the
row persists, so a re-attached tracker replays the event and finalizes.
Excluding those rows (Codex review, round 1) left them `Running` forever,
because reconcile now defers running rows. For each such row, shutdown:

- aborts only the tracker (the exit-monitor handle); the row stays `Running`;
- sends **no** cancellation;
- does **not** commit WIP, because the worker still owns the worktree;
- persists the tracker's **pushed** sequence as the cursor, then waits
  `WORKER_HANDOFF_FLUSH_GRACE` (250 ms) once.

The acknowledged cursor alone is wrong in **both** directions, which Codex
found in round 1. The tracker pushes a whole batch into the MsgStore and only
then acknowledges. The JSONL writer is a separate task reading the broadcast.
If the tracker is aborted between push and acknowledgement, the pushed lines
still reach disk, and re-attach from the older cursor writes them twice. So
each tracker publishes a `worker_pushed_sequences` atomic. It advances only
across pure-output events (`worker_event_is_output`) and stops before an
interaction or terminal event, whose coordinator-side effects a restart can
lose and which must be replayed. Handoff writes that value through the
monotonic `acknowledge_sequence`, and the grace lets the writer reach it. A
hard crash skips all of this, so up to one batch may be lost or repeated.
That residual is accepted.

A failed ownership lookup falls through to the old local rules. That is
conservative: it still routes a worker row through `stop_execution`.

## Re-attach: three things must line up

`reattach_worker_executions` runs level-triggered at boot, over **every**
`Running` row with a non-terminal worker job, not over a list saved at
shutdown. So it also covers a crash where the shutdown hook never ran. For
each row it:

1. **Seeds a fresh MsgStore** with `load_raw_log_messages`. Executor
   normalizers are stateful, and entry indexes come from the whole stream, so
   the store must hold the whole history before new output. Otherwise the chat
   restarts at entry 0 and patches collide.
2. **Tells the raw-log writer how many seeded lines to skip**
   (`spawn_stream_raw_logs_to_storage(.., skip_raw_history)`). The writer reads
   `history_plus_stream`, so without the skip it rewrites the seeded history
   into the same file. The skip counts messages *retained by the store* (it evicts
   its oldest history past about 100 MB; counting what was loaded instead
   silently dropped new output, Codex round 1) instead of relying on
   subscription timing, because seeded messages are always first. That keeps
   it race-free however soon the tracker pushes.
3. **Starts the tracker at `last_event_sequence`** (`resume_from`), reusing
   the seeded store. Never start from 0: the journal is a 4096-event ring, so
   its head may already be gone, which would turn a healthy re-attach into a
   replay gap.

Dispatch and re-attach share `start_execution_log_pipeline` (normalizer and
writer) and `spawn_execution_stage_tracker`. That is deliberate. A second copy
of "which actions get normalized, which runs write their own raw log" is the
kind of second resolution rule that drifts.

The pipeline-stage reset and SpecKit provisioning stay in `start_execution`
only. They belong to a *new* turn, not a resumed one.

## Boot reconcile must not finalize a running row

`ExecutionReconciler` runs before re-attach. It used to copy terminal
evidence straight onto the process row. For a job that finished while the
coordinator was down, that silently skipped `finalize_remote_execution`:
commit, next action (cleanup script), queued follow-up, notification and the
remediation trigger. It also dropped the final output that had not yet been
acknowledged.

Now a terminal summary whose row is still `Running` counts as
`jobs_deferred`. Only the observed sequence is recorded; the row and the
dispatch state are left alone. The re-attached tracker replays the terminal
event and finalizes through the one normal path. Leaving the dispatch state
non-terminal is load-bearing, because it is exactly what makes the row
re-attachable. Rows that are already terminal keep the old conflict and
idempotence rules.

Alternative rejected: "keep reconcile writing, then run finalization at
boot". That is a second finalization path, and it still loses the tail of the
output.

## A replay gap is not an ending

When `events` reports a gap and the inventory shows the **same identity**
(worker, execution, job id, digest) still non-terminal
(`replay_gap_job_is_live`), the tracker:

- marks the job `output_complete = false`;
- pushes a stderr notice naming the lost event range;
- continues from `earliest_available - 1`.

Previously any gap without terminal evidence became `Indeterminate`. That
declared a still-running agent finished-with-unknown-outcome just because the
coordinator had been away long enough for the ring to wrap. Terminal-evidence
recovery and the unreachable/mismatch → `Indeterminate` rules are unchanged.

## Not covered (by design)

- **Coordinator-local work** (placement `Local`, or no cluster). Claude and
  Codex talk to VK over stdio, so their processes cannot outlive the server.
  It is still `Interrupted` + WIP commit + the opt-in
  `resume_interrupted_on_startup` (config-only, default off). Surviving there
  still needs the Tier-3 runner split.
- **Pending approvals/questions** at the instant of restart are in memory
  (`route_worker_interaction`). They are resolved by the worker's interaction
  timeout and fail-closed policy.
- **Queued follow-ups** (`QueuedMessageService`) are in memory and still lost.
- **Worker-owned persistent runs** keep no coordinator-side JSONL
  (`writes_own_raw_log` is keyed on `is_persistent()`, not on who runs it). A
  re-attached poller therefore shows only post-restart output. This predates
  the change.
- Stopping the coordinator no longer stops remote agents. Stop and restart are
  both SIGTERM, so they cannot be told apart. Use UI Stop or stop the worker.

## Contributed by

- vk/80c1-tasks-should-sur
