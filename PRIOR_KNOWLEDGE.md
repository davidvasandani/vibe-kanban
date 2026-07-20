# Prior Knowledge: VK Queued Messages Hang

Task: `9f36-vk-queued-messag`

The project knowledge base was searched through `wiki/INDEX.md`,
`docs/knowledge-base/INDEX.md`, and their topic pages for queued messages,
follow-ups, execution lifecycle, finalization, session state, scratch drafts,
and concurrency races.

## Agent process lifecycle

Source: `wiki/agent-process-lifecycle.md`

- One coding-agent turn maps to one `ExecutionProcess`; process tracking and
  exit-monitor facilities are keyed by execution id, while queued follow-up
  dispatch occurs during the exit monitor's finalization work.
- Finalization may commit changes, select a next action, dispatch a queued
  follow-up, update the database, and only later finish process cleanup. Code
  must not equate frontend-visible output completion with a still-available
  finalization consumer.
- The documented warm-process flow requires "park before finalization" because
  queued-follow-up dispatch can immediately start another turn. This establishes
  that finalization ordering is a concurrency-sensitive contract and that the
  follow-up path should reuse existing execution dispatch rather than create a
  separate frontend-only mechanism.
- Lifecycle ownership changes must avoid invisibility gaps and must not hold
  shared registry locks across awaited process operations. The same principles
  apply when coordinating queue insertion with completion consumption.
- A completed coding-agent row can coexist with a still-live warm OS process.
  Queue eligibility must therefore use the execution lifecycle contract, not
  merely process existence or a frontend running indicator.

## Knowledge not currently recorded

No existing knowledge page documents the in-memory `QueuedMessageService`, its
one-message-per-session replacement rule, the queue HTTP contract, or how the
frontend queue cache converges after backend consumption. Those details must be
derived from the current code and tests during planning.

## Implications for specification and planning

1. Reuse the existing queued-follow-up dispatcher and respect finalization,
   chained-action, setup-script, and warm-process ordering.
2. Audit every early-finalization branch for work normally performed by the
   bypassed finalization block; "already finalized" must not also mean "skip
   pending handoffs."
3. The report's `0 files changed` state points directly to cleanup-skip behavior,
   which should be tested before broadening the change to a speculative race fix.
