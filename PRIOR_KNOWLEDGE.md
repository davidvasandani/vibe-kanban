# Prior knowledge — vk/80c1-tasks-should-sur

Distilled from the VK wiki (`wiki/`), the homelab knowledge base
(`homelab/knowledge-base/`), prior SpecKit research, and the homelab
constitution. Read-only recall; nothing below was changed by this stage.

## Already survives a restart (reuse, don't reinvent)

- **Persistent processes** (dev servers, background helpers, pollers) running
  on the coordinator are detached on shutdown (`detach_execution_for_handoff`)
  and re-adopted by `pgid` on boot (`try_adopt_execution`)
  — `wiki/agent-process-lifecycle.md` "What already survives restarts",
  `wiki/vk-pollers.md` "A poller is a background helper, deliberately".
- A poller's **deadline lives in its own process group** (a watchdog), so
  re-adoption preserves it. A server-owned timer would reset on restart
  (`wiki/vk-pollers.md`, "A deadline belongs to the process group").
- **Interrupted coding-agent turns** get `Interrupted` status, a WIP commit,
  and an optional one-shot resume on boot (`resume_interrupted_on_startup`,
  default off; a resume is never itself resumed).
- The **worker drain** is in the homelab distributor
  (`modules/vibe-kanban-rebuild.nix` ~3257-3506). SIGUSR1 closes admission;
  the worker restarts only while `/health.drain_safe`, i.e. no active job
  (pollers included); SIGUSR2 reopens it. This protects "coordinator soft
  restarts" from worker replacement.

## Deferred designs, and why

- Tier-3 restart survival for **coordinator-local** agents was deferred
  (`homelab/specs/vk/1a64-coding-agent-pro/research.md`). The options are
  exec-in-place upgrade and a supervisor/runner split. The cluster worker
  already *is* a runner split for worker-placed workspaces. So the cheap,
  correct fix is to stop the coordinator undoing that split.
- A VK-owned wake-up scheduler (VAS-283 option B) is out of scope and must not
  be reopened incidentally (`wiki/vk-pollers.md`).

## Cluster protocol facts that constrain the design

- Worker output is an in-memory **ring-buffer journal**, capacity 4096 events
  (`crates/worker/src/journal.rs`, `DEFAULT_JOURNAL_CAPACITY`). Acknowledgement
  does not trim it. A replay past the trimmed front returns a replay gap.
- The coordinator persists the acknowledged cursor
  (`execution_worker_jobs.last_event_sequence`). The tracker acknowledges only
  after pushing to the MsgStore, and never acknowledges terminal events until
  the process row is persisted (`wiki/worker-journal-agent-stream-boundary.md`,
  and `container.rs`).
- Only worker `Stdout` bytes and `LogMsg::Stdout` may reach agent stdout.
  `Structured` metadata goes through `classify_worker_structured`
  (constitution IX 0.37.0).
- A worker lease proves process liveness, not turn liveness, for
  signal-driven executors (`wiki/agent-process-lifecycle.md`).
- `finalize_task` is the single convergence point for notifications and the
  auto-remediation trigger. `Interrupted`/`Killed`/`Indeterminate` never
  trigger remediation (`wiki/auto-error-remediation.md`).
- A worker restart interrupts its jobs rather than resuming them
  (`crates/worker/src/lib.rs`, recovery comment).

## Governing principles

- homelab constitution 6 (explicit, actionable failures; level-triggered
  reconciliation). Principle 89: a fast restart still destroys every in-flight
  session, so fix it at the boundary we own. Principle 90 (probes). Principle
  126 (a deploy restarts a service; it does not take it down).
- homelab constitution entry "Vibe Kanban worker recovery (vk/611f)": preserve
  active processes and document maintenance stops.
- VK repo rule: the dispatch and re-attach log pipelines must not diverge. A
  second resolution rule for the same fact is a recurring defect class
  (`wiki/vk-pollers.md`, "`PollerSpec` is retained, not re-derived").

## Gaps found (no prior page covers them)

- Coordinator shutdown **cancels** worker jobs (`kill_all_running_processes`
  → `stop_execution`).
- No re-attach of worker trackers after boot. Worker-owned rows that were left
  `Running` are orphaned. Boot reconcile writes terminal evidence without
  finalization.
