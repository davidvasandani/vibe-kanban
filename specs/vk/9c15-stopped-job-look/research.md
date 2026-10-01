# Research: vk/9c15-stopped-job-look

## Incident evidence (think4, 2026-10-01)

- `journalctl -u vibe-kanban-worker`: stop/start at 08:44:49–08:44:55 UTC
  with **no** SIGUSR1 drain (the later 09:06 restart was drained).
- The worker recovery record
  `/var/lib/vibe-kanban-worker/64369241-….json` shows `state: interrupted`,
  `last_sequence: 5`, `terminal.observed_at 08:44:55`. The run had produced
  far more events than that. The next run of the same workspace, the user's
  Resume, reached `last_sequence: 2549`, which shows the scale.
- UI recording: Stop spinner until it was clicked, then the "interrupted by a
  vibe-kanban restart" banner. So the stored status became `interrupted`
  only after the manual Stop.

## Why the tracker hung

`WorkerJob::persist` runs on transitions only, so the persisted
`last_sequence` is the one from `Running`. `EventJournal::recover` appends the
terminal event at that value + 1. `replay_after(cursor)` with `cursor ≫ 6`
yields no events and no gap. The tracker's loop has no branch for an empty,
non-gap batch, so it polls forever.

## Decision: detect regression on the coordinator

- **Chosen**: treat `latest_available < cursor` as a lost stream and resolve
  it from the inventory, with exact identity and same-stream sequence.
  - It covers every way the worker loses its journal.
  - It needs no protocol change, and it works with workers already deployed.
- **Rejected: persist `last_sequence` on every event or acknowledgement.**
  It narrows the window but cannot close it: the worker can die between
  serving a batch and saving. It also adds a disk write per batch on the
  NFS-adjacent state dir.
- **Rejected: have the worker flag `replay_gap` for `after > latest`.**
  This routes through the replay-gap path, whose evidence rule
  (`summary.last_sequence >= cursor`) correctly rejects the recovered record.
  The result would be `Indeterminate` instead of `Interrupted`, losing
  Resume, and it needs a worker deploy.
- **Rejected: frontend timeout heuristics.** They would contradict XXX, under
  which the UI follows authoritative state.
