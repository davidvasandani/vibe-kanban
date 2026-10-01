# SPEC: A worker restart must not leave a stopped job looking Running

Task: `vk/9c15-stopped-job-look` ("Stopped Job looks Running").

## Problem

After a VK restart, a stopped job kept showing as running in the chat. The
**Stop** button kept its spinner and the last tool call kept its loading
placeholder. Only after the user pressed **Stop** did the UI switch to "This
run was interrupted by a vibe-kanban restart" with **Resume**.

Evidence from production (think4 worker, workspace `8b57b52f…`, execution
`64369241-f659-4acf-9611-9ab6e1919b97`):

- `vibe-kanban-worker` on think4 was restarted at 08:44:49 UTC **without a
  drain** (no SIGUSR1). Its agent child died with it.
- On boot the worker recovered the job from its recovery store as
  `interrupted` (`observed_at 08:44:55`). The recovered summary has
  `last_sequence: 5`.
- The coordinator row stayed `Running` until the user pressed Stop about 25
  minutes later.

## Root cause

The worker persists a job's `JobSummary` (`WorkerJob::persist`) only on
state transitions. During a long run the on-disk `last_sequence` stays at
the value from the `Running` transition (5 here), while the journal goes on
to thousands of events, all delivered to and acknowledged by the coordinator.

`ExecutionSupervisor::with_recovery_and_drain` rebuilds the journal with
`EventJournal::recover`. That places the synthetic terminal event at
`persisted last_sequence + 1` (sequence 6). The coordinator's tracker
(`track_worker_msgs_in_store`) polls `events(after = cursor)` with its real
cursor (thousands). `replay_after` returns:

- no events (`6 <= cursor`);
- no replay gap (`cursor + 1 >= earliest_available`);
- `latest_available = 6 < cursor`.

The tracker treats that as "nothing new yet" and polls forever. The terminal
event is never observed and the row stays `Running`. Boot reconciliation
(`ExecutionReconciler`) defers running rows with terminal summaries to that
same tracker (see `wiki/coordinator-restart-handoff.md`), so a coordinator
restart hits the same hang. The frontend is correct: it renders the row's
`running` status.

## Requirements

- **R1 — Detect journal regression.** A batch whose `latest_available` is
  below the coordinator's cursor proves the worker lost the journal the
  cursor was read from (a worker restart). In normal operation every cursor
  value came from that worker's own events, so `latest_available >= cursor`
  always holds. The tracker must stop treating this batch as "no new events".
- **R2 — Finalize from matching terminal evidence.** On regression, consult
  the worker inventory. If a summary matches this dispatch exactly (worker,
  execution, worker job id, request digest), is terminal with consistent
  terminal evidence (state/evidence pair as in replay-gap recovery), and its
  `last_sequence` equals the batch's `latest_available` (same journal
  generation), finalize the row through the **normal** terminal path:
  persist the dispatch state and the process status, acknowledge, run
  `finalize_remote_execution`, and finish the MsgStore. For a worker restart
  the status is `Interrupted`, so the existing Resume affordance appears with
  no Stop click.
- **R3 — Honest output.** Mark the job `output_complete = false`. Push one
  stderr notice saying the worker restarted and output after the
  coordinator's cursor may be missing.
- **R4 — No matching evidence.** If the inventory is reachable but has no
  matching terminal summary, the regressed job's state cannot be trusted.
  Mark the row `Indeterminate` (the existing unknown-outcome rule) and
  finalize, rather than leaving it `Running`. If the inventory or database
  lookup fails, retry with the tracker's existing backoff and never infer a
  terminal state.
- **R5 — Do not touch healthy paths.** Batches with
  `latest_available >= cursor`, replay gaps, interactions, handoff and
  re-attach keep their current behavior.
- **R6 — Tests.** Unit tests cover the pure regression predicate and the
  evidence matcher: identity mismatches, non-terminal or contradictory
  summaries, journal-generation mismatch, and all four terminal states.

## Non-goals

- Persisting the worker's `last_sequence` more often. It narrows the window
  but cannot close it, because the worker can die between delivering a batch
  and saving. The coordinator-side rule covers every case.
- Changing the frontend; it already renders the row status faithfully.
- Repairing historical rows. The incident row was already finalized by the
  user's Stop.
- A job missing entirely from the worker's inventory (state dir wiped). That
  keeps today's `events` error/retry behavior.

## Acceptance

- `cargo test -p local-deployment` passes, including the new tests.
- `pnpm run backend:check`, `cargo clippy` and `pnpm run format` are clean.
- Reasoned trace: with the incident's values (cursor ≫ 6, recovered
  `Interrupted` summary at `last_sequence = 6`), the tracker finalizes the row
  as `Interrupted` on its first poll after the worker restarts.
