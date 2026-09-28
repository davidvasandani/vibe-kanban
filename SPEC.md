# SPEC — Tasks survive VK restarts

Task: `vk/80c1-tasks-should-sur`

## Problem

A VK deploy restarts the coordinator (`vibe-kanban-dev` on think2) with no
drain. Every running task stops, including parent agents, their sub-task
workspaces, and their pollers. The user then has to notice and restart each one.

On a cluster deployment almost every agent turn already runs on a **worker**
(think1/3/4/5), not on the coordinator. The worker is a separate process that
has no reason to die when the coordinator restarts. The coordinator kills the
work itself, in two ways:

1. **Shutdown cancels worker jobs.** `kill_all_running_processes`
   (`crates/local-deployment/src/container.rs`) calls
   `stop_execution(.., Interrupted)` for every non-persistent running row.
   For a worker-owned row that sends a `CancellationRequest` to the worker. So
   SIGTERM on the coordinator actively kills remote agent turns. It then
   commits WIP into a worktree the agent may still be writing.
2. **Boot never re-attaches.** Nothing re-attaches after the boot
   `ExecutionReconciler`. Worker-owned rows that were left `Running` (which
   includes pollers and background helpers, since they are detached rather
   than cancelled) never get their worker event tracker back. It is started
   only from `dispatch_execution`. So their output never reaches the chat,
   their terminal state never lands, and `finalize_remote_execution` never
   runs. The reconciler writes terminal evidence straight onto the row for a
   job that finished while the coordinator was down. That skips the commit,
   the next action (cleanup script), the queued follow-up and the
   notification, and drops the final output that was not yet acknowledged.

Worker restarts are already handled. The homelab distributor closes admission
(SIGUSR1) and restarts a worker only when `/health.drain_safe` is true, which
means no active job, pollers included. It then reopens the worker (SIGUSR2). A
worker's sessions therefore already "reload after the task is idle".

## Goals

1. A coordinator restart (deploy, `systemctl restart`, crash) does **not** stop
   worker-owned executions. This covers coding-agent turns, review turns,
   setup/cleanup scripts, background helpers, pollers and dev servers.
2. After the coordinator comes back it **re-attaches** to every worker-owned
   execution still marked `Running`:
   - the chat shows the history recorded before the restart, and then the
     output produced during and after it;
   - terminal state is recorded from the worker's evidence;
   - normal finalization runs: commit, next action, queued follow-up,
     notification and auto-remediation trigger.
3. This holds for parent sessions, the sub-task workspaces they started, and
   pollers alike. It follows because each of them is simply a worker-owned
   execution.
4. Output lost while the coordinator was down is reported honestly and never
   silently. If the worker's journal wrapped past the acknowledged cursor, the
   chat shows a visible "output was lost" notice, the job is marked
   `output_complete = false`, and tracking continues if the worker says the job
   is still live.

## Non-goals

- **Coordinator-local executions** (placement `Local`, or a non-cluster
  deployment). Claude and Codex talk to VK over stdio, so the process cannot
  outlive its peer without a supervisor/runner split. Those keep today's
  behavior: marked `Interrupted`, WIP committed, and resumed once on boot when
  `resume_interrupted_on_startup` is on.
- **Changing the worker drain**, which already waits for idle.
- **Approvals/questions pending at the moment of restart.** The in-memory
  approval is lost. The worker's interaction timeout and fail-closed
  disconnect policy resolve it. This is documented, not fixed.
- **Queued follow-up messages** (`QueuedMessageService`, in memory). A message
  queued before a restart is still lost; it is recorded as a follow-up.
- Changes to the homelab module: the fix is entirely coordinator-side.

## Design

### Shutdown: hand off, don't cancel

In `kill_all_running_processes`, first check each running row for a
non-terminal `ExecutionWorkerJob`. If there is one, **hand it off**:

- abort the tracker task (the exit-monitor handle), leaving the row `Running`
  and the job row untouched;
- do **not** call `stop_execution`, and do **not** commit WIP, because the
  worker still owns the worktree;
- log `Leaving worker-owned execution … running across coordinator restart`.

After the loop, if anything was handed off, sleep a short bounded interval
(250 ms). This lets the raw-log writer flush lines already pushed to the
MsgStore, because the tracker acknowledges a sequence right after pushing.
Local processes keep today's behavior: persistent ones detach, the rest are
interrupted.

### Boot reconcile: defer running rows to the tracker

In `ExecutionReconciler::apply_worker_evidence`, when the job is terminal on
the worker but the process row is still `Running`, record the observed worker
sequence only. Leave both the row and the job's dispatch state alone, and
count it as `jobs_deferred`. The re-attached tracker replays the terminal
event itself and finalizes through the normal path. Rows that are already
terminal keep today's conflict and idempotence rules. So do missing,
mismatched and unknown jobs.

### Boot re-attach

A new `ContainerService::reattach_worker_executions()` (default no-op;
implemented by `LocalContainerService`) runs after `cleanup_orphan_executions`
in both boot paths (`crates/server/src/main.rs`,
`crates/server/src/startup.rs`). For every `Running` process that has a
non-terminal worker job:

1. Seed a fresh `MsgStore` with the persisted raw history
   (`load_raw_log_messages`).
2. Start the same log pipeline as `start_execution`: normalizer for agent
   actions, raw-log writer (non-persistent runs only), pipeline-stage tracker
   for coding agents. The writer **skips the seeded history**, so nothing is
   duplicated on disk.
3. Start the worker event tracker from the job's persisted
   `last_event_sequence` instead of `0`.

Extract that shared pipeline from `start_execution` into one helper, so dispatch
and re-attach cannot drift apart.

### Replay gap on a live job

When `events` returns a replay gap, the tracker reads the worker inventory:

- it finalizes from terminal evidence if there is any (unchanged);
- if the inventory shows the same job identity still **non-terminal**, it now
  marks the job `output_complete = false`, pushes a visible stderr notice
  naming the lost range, and continues from `earliest_available - 1`;
- otherwise it marks the row `Indeterminate` (unchanged).

## Acceptance criteria

- **AC1** Coordinator shutdown sends no cancellation to a worker for an
  execution with a non-terminal worker job. The row stays `Running` and no WIP
  commit is attempted for it.
- **AC2** Local executions are unchanged: persistent ones detach, and coding
  agents become `Interrupted` with WIP committed.
- **AC3** Boot reconcile leaves a `Running` row whose worker job is terminal
  untouched and counts it as deferred. Rows that are already terminal behave as
  before.
- **AC4** After boot, a re-attached execution's tracker polls from the
  persisted cursor. Its MsgStore contains the pre-restart history followed by
  new output, and the on-disk log gains only the new lines.
- **AC5** A terminal event received by a re-attached tracker updates the row
  and runs `finalize_remote_execution`.
- **AC6** A replay gap on a still-running job produces a visible notice, sets
  `output_complete = false` and keeps tracking, instead of marking the row
  `Indeterminate`.
- **AC7** `cargo test --workspace` and `pnpm run check`/`lint` pass. New unit
  tests cover AC1, AC3, AC4 (writer skip and cursor) and AC6.

## Risks

- **Output written between push and flush at a crash (SIGKILL).** The
  shutdown hook never runs in that case, so a few lines may be missing. The
  cursor is never moved backwards, so nothing is duplicated.
- **A worker unreachable at boot.** Its rows keep a tracker that retries with
  backoff, the same as a runtime network blip. This is better than today,
  where the row stays `Running` with nothing watching it.
- **Explicit maintenance.** Stopping the coordinator no longer stops remote
  agents. Use the UI Stop or stop the worker when that is intended.
