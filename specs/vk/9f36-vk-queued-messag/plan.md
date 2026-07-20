# Implementation Plan: Queued Follow-up After No-change Run

**Spec**: `./spec.md`
**Status**: Ready for implementation

## Technical Context

The Rust `LocalContainerService` exit monitor owns completion, commit detection,
next-action selection, queue consumption, and finalization. A successful coding
run with no commits skips its cleanup next action and manually finalizes before
the general `should_finalize` queue block. Queue state and API/frontend contracts
do not need to change.

## Architecture & Approach

1. Extract scratch deletion plus `start_queued_follow_up` into a local boolean
   helper that reports whether dispatch succeeded.
2. In the no-change cleanup-skip branch, take the queued message and invoke the
   helper before manual finalization.
3. Finalize only when the message is absent, cancelled concurrently, or cannot
   start. Keep `already_finalized` so the later finalization block is not run
   twice.
4. Reuse the helper in the existing normal consumer.
5. Add focused tests for the skipped-cleanup decision and run relevant Rust
   validation plus repository formatting.

## Data Model

No data-model or persistence changes.

## Contracts

No HTTP, generated type, or frontend contract changes.

## Research Notes

See `./research.md` for root-cause evidence and rejected broader changes.

## Constitution Check

The smallest fix reuses shipped execution machinery (I, III, VI), adds focused
contract coverage (II), and closes an asynchronous handoff before early
finalization (XII). No deviations or new dependencies.

## Risks & Dependencies

- Cancellation can race between observing and taking a message; absence at take
  time safely falls back to finalization.
- Scratch deletion is best effort, consistent with the existing normal consumer.
- Starting a queued follow-up after the task was manually finalized would be too
  late, so dispatch must occur before `finalize_task`.
