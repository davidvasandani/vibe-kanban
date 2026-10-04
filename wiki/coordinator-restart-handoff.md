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
- ends the store, waits (bounded by `WORKER_HANDOFF_FLUSH_TIMEOUT`, 5 s total)
  for its raw-log writer to drain, and only then persists the tracker's
  **pushed** sequence as the cursor. If the writer does not finish in time,
  the cursor is left alone. Residual (Codex round 6): the tracker's own
  per-batch acknowledgements already ran ahead of the writer, so a writer
  stalled past the timeout by storage can still lose the lines it never
  wrote. Closing that means gating acknowledgement on writer progress, which
  was deferred as a redesign. Under normal storage the writer drains in
  milliseconds.

The acknowledged cursor alone is wrong in **both** directions, which Codex
found in round 1. The tracker pushes a whole batch into the MsgStore and only
then acknowledges. The JSONL writer is a separate task reading the broadcast.
If the tracker is aborted between push and acknowledgement, the pushed lines
still reach disk, and re-attach from the older cursor writes them twice. So
each tracker publishes a `worker_pushed_sequences` atomic. It advances only
across pure-output events (`worker_event_is_output`) and stops before an
interaction or terminal event, whose coordinator-side effects a restart can
lose and which must be replayed. Handoff writes that value through the
monotonic `acknowledge_sequence`, after awaiting the writer. A fixed sleep
was the first attempt; Codex round 4 pointed out it proves nothing under slow
storage, so the writer task handle is now kept (`register_raw_log_writer`,
`worker_log_writers`) and awaited. A
hard crash skips all of this, so up to one batch may be lost or repeated.
That residual is accepted.

A failed ownership lookup falls through to the old local rules. That is
conservative: it still routes a worker row through `stop_execution`.

## Re-attach: three things must line up

`reattach_worker_executions` runs level-triggered at boot, over **every**
`Running` row that has a worker job, not over a list saved at shutdown. So it
also covers a crash where the shutdown hook never ran. For each row it:

1. **Seeds a fresh MsgStore** with `load_raw_log_messages`. Executor
   normalizers are stateful, and entry indexes come from the whole stream, so
   the store must hold the whole history before new output. Otherwise the chat
   restarts at entry 0 and patches collide.
2. **Subscribes the raw-log writer before anything else can push**
   (`spawn_resumed_raw_log_writer`). The writer replays history and then goes
   live, so it must skip the seeded lines already on disk. The skip count and
   the replayed snapshot come from one lock
   (`MsgStore::history_plus_stream_counting`). Two earlier versions were
   wrong. Counting what was *loaded* ignored the store's eviction past about
   100 MB (Codex round 1). Counting *retained* history before a later
   subscription let the tracker and normalizer push and evict in between
   (round 2). Either way the writer silently skipped new output.
3. **Starts the tracker at `last_event_sequence`** (`resume_from`), reusing
   the seeded store. Never start from 0: the journal is a 4096-event ring, so
   its head may already be gone, which would turn a healthy re-attach into a
   replay gap.

Dispatch and re-attach share `start_execution_log_pipeline` (the normalizer,
plus the writer when `spawn_raw_writer` is set) and
`spawn_execution_stage_tracker`. They also share one
`execution_process::writes_own_raw_log` rule. Re-attach passes
`spawn_raw_writer = false` only because it has already started its writer.
A second copy of "which actions get normalized, which runs write their own
raw log" is the kind of second resolution rule that drifts.

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

## A journal behind the cursor is a worker restart

The worker writes a job's `JobSummary` to its recovery store only on state
transitions, so the on-disk `last_sequence` stays at the value from
`Running` for the whole run. On boot, `with_recovery_and_drain` marks every
non-terminal job `Interrupted`, and `EventJournal::recover` puts that
terminal event at `persisted last_sequence + 1`. For a long run that is far
below the coordinator's cursor. `replay_after(cursor)` then returns no
events, no gap and `latest_available < cursor`. The tracker read this as
"nothing new" and polled forever. The row stayed `Running` (Stop spinner)
until a manual Stop. Incident: think4 worker restarted without a drain,
2026-10-01, recovered at `last_sequence: 5`, coordinator cursor in the
thousands.

Every cursor value is a sequence that journal served, so a live journal
never reports `latest_available < cursor`. That makes the regression
unambiguous (`worker_journal_regressed`). The tracker then reads the
inventory:

- **Exact-identity summary, plus `last_sequence == latest_available`**
  (`journal_regression_terminal_evidence`): the second check confirms the
  summary describes the journal now being served. The tracker marks output
  incomplete, pushes one stderr notice and sets `terminal`, then falls into
  the **normal** terminal block. A worker restart therefore yields
  `Interrupted`, and the chat shows a stopped-run row with a Restart button
  (`SessionChatBox`'s `interruptedNotice` banner — see
  [[session-chat-box-stopped-banner]]).
- **Readable, but no matching summary:** `Indeterminate` (skipped if the
  user already stopped the row), then finalize.
- **Lookup error:** back off and re-poll. Never infer an outcome.

The replay-gap evidence rule cannot be reused here. It requires
`summary.last_sequence >= cursor`, which the recovered summary fails by
construction. Flagging the case as a gap on the worker would therefore have
produced `Indeterminate` and lost Resume. Persisting `last_sequence` more
often was also rejected. It only narrows the window, because a worker can die
between serving a batch and saving it. Both helpers share
`terminal_summary_states` with replay-gap recovery.

## Pending approvals are replayed, not dropped

An approval waiter lives only in memory (`route_worker_interaction`), and the
worker holds an unanswered interaction for up to **10 hours**, fail-closed.
If the persisted cursor passed an unanswered `InteractionRequested`, the task
would stall for hours after a restart, with no prompt shown (Codex round 2).
So the tracker keeps an `UnresolvedInteractions` set of request sequences.
The **DB** cursor (`durable_worker_cursor`) and the pushed cursor stay just
before the earliest one until its response is delivered. The worker ack
still advances, because it is bookkeeping and does not trim the journal.
Re-attach therefore replays the request and the approval reappears. Replay
is safe because the worker answers an already-completed interaction with
success. The cost is that any output after an unanswered request is written
again after a restart; an agent blocked on approval rarely emits any.

## Not covered (by design)

- **Coordinator-local work** (placement `Local`, or no cluster). Claude and
  Codex talk to VK over stdio, so their processes cannot outlive the server.
  It is still `Interrupted` + WIP commit + the opt-in
  `resume_interrupted_on_startup` (config-only, default off). Surviving there
  still needs the Tier-3 runner split.
  - **Work that must run on the coordinator host no longer has to run *in*
    the coordinator.** think2 now also runs a `vibe-kanban-worker`
    (homelab `colocatedWorker`, vk/ec43-run-a-vibe-kanba). It is its own
    systemd unit, cgroup and account (`vibe-kanban`), at
    `http://172.16.100.102:8096`, so a workspace placed on it gets the
    handoff above. Pick it in the create dialog when a task needs think2.
    Automatic placement mostly avoids it:
    - the score uses host `load_1m`, which includes the coordinator's own
      load;
    - its node ID sorts last, so it loses ties;
    - it advertises `CLAUDE_CODE` only, because think2's `vibe-kanban`
      account holds the canonical Codex login.
  - **Existing `Local` workspaces cannot be moved onto a worker.**
    `PATCH /affinity` refuses `placement_state = 'local'` in SQL
    (`WorkspacePlacement::reassign`). With `restart_running` it stops the
    agent *before* failing. Their worktrees also live under
    `/var/tmp/vibe-kanban/worktrees`, not the shared store. In cluster mode,
    new workspaces are never placed `Local`: automatic placement picks a
    worker or fails. So a follow-up that must survive restarts goes in a new
    workspace. Moving one would need an app change that copies the worktrees
    into the shared store.
  - **A worker that is draining (SIGUSR1) still looks schedulable.** The
    heartbeat does not carry `admission_draining`. A dispatch sent during a
    distribution drain window is refused by the worker and fails, rather
    than being placed elsewhere. This is true of every worker.
- **An unanswered interaction lost in a replay gap.** If the worker's journal
  wraps past a pending `InteractionRequested` while the coordinator is away,
  the request itself is gone, and no coordinator-side state can recreate its
  waiter. The same was already true of any runtime gap. Recovering it needs a
  worker API that lists pending interactions.
- **Queued follow-ups** (`QueuedMessageService`) are in memory and still lost.
- **Worker-owned persistent runs** keep no coordinator-side JSONL
  (`writes_own_raw_log` is keyed on `is_persistent()`, not on who runs it). A
  re-attached poller therefore shows only post-restart output. This predates
  the change.
- Stopping the coordinator no longer stops remote agents. Stop and restart are
  both SIGTERM, so they cannot be told apart. Use UI Stop or stop the worker.

## Contributed by

- vk/80c1-tasks-should-sur
- vk/ec43-run-a-vibe-kanba (colocated worker on the coordinator host)
- vk/9c15-stopped-job-look (journal regression after an undrained worker restart)
- vk/556e-start-stopped-se (stopped-run banner UI, see [[session-chat-box-stopped-banner]])
