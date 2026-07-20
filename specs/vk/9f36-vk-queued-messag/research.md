# Research Notes

## Root cause

The reported screenshot shows `0 files changed`. In the exit monitor, a
successful coding-agent execution computes `should_start_next = false` when
neither automatic commit nor execution-created commits exist. It then skips the
cleanup script, calls `finalize_task`, and sets `already_finalized = true`. The
only general `take_queued` block is guarded by `!already_finalized`, so the
message is never consumed.

## Decision

Consume/start a queued follow-up in the cleanup-skip branch before manual
finalization, sharing scratch/start behavior with the normal consumer.

## Rejected: queue API/state-machine redesign

The initial hypothesis was a completion/submission race. The concrete evidence
and branch trace explain the report without API or synchronization changes.
Broadening the queue protocol would add risk and violate the smallest-change
principle without evidence it is needed for this task.

## Dependencies

No new dependencies.
