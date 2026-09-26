# Prior knowledge: vk/5276-debug-unrecogniz

Distilled from the knowledge bases (read-only). Sources: `vibe-kanban/wiki/` (INDEX plus
pages matching cancel/structured/worker/stdout/unrecognized) and
`homelab/docs/knowledge-base/vibe-kanban-*`. Neither knowledge base mentions
`cancellation_phase`, the `Structured` worker payload, or the "Unrecognized JSON
message" rendering. This defect is new territory.

## Relevant background
- **`wiki/agent-process-lifecycle.md`**: cluster execution splits authority
  between the worker's journal and the coordinator's projection of it. Terminal
  evidence (Completed, Killed, Interrupted, Indeterminate) comes from the worker, and the
  coordinator must not invent it. *Implication:* cancellation phases are
  progress bookkeeping, not lifecycle evidence. Dropping them from the chat does
  not weaken any terminal-state rule, because the `Killed` event still follows.
- **`wiki/agent-process-lifecycle.md` (output cleanup)**: stdout/stderr draining
  is bounded and ordered before terminal journal closure. *Implication:* the
  journal interleaves agent bytes with product events in one ordered stream. The
  coordinator is the one place that separates them.
- **`wiki/awaited-stream-settlement.md`** and **`coordinator-nfs-load.md`** cover
  other coordinator-side stream handling. Neither touches `Structured`
  payloads.
- **`homelab/docs/knowledge-base/vibe-kanban-worker-start-recovery.md`**: worker
  launch failures are the source of `worker_error`. Those reasons are often the only
  diagnosis of a failed start, so they must stay visible (constitution XI).

## What this task builds on
- The existing coordinator convention: coordinator-authored worker diagnostics
  in the poll loop are pushed as `LogMsg::Stderr`. Examples are the replay-gap line and
  "Worker reported an indeterminate execution".
- The existing `worker_event_tests` module next to `push_worker_bytes`.
