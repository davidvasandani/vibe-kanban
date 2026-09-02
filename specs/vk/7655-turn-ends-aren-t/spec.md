# Feature Specification: Turn completion clears the running composer

**Feature dir**: `specs/vk/7655-turn-ends-aren-t/`
**Task id**: `vk/7655-turn-ends-aren-t`
**Status**: Clarified

## Summary

Ensure that a workspace chat returns to its idle composer state after an agent
turn terminates. A user must not see a perpetually spinning Stop control after
the assistant's completed turn is visible, while genuinely active processes
must remain stoppable.

## User Stories

- As a workspace user, I want the composer to return to Send/continue when an
  agent turn finishes so that I can confidently start the next turn.
- As a workspace user, I want Stop to remain available while work is genuinely
  active so that I can cancel it.
- As a user whose connection or service briefly recovers, I want the composer
  to reconcile automatically so that a missed lifecycle event does not require
  a page refresh.
- As an operator, I want an execution without positive liveness evidence to be
  classified truthfully so that the UI cannot claim it is running forever.

## Functional Requirements

- **FR-1**: The workspace composer must show Stop only while the latest
  authoritative process state for the selected session contains a relevant
  active execution.
- **FR-2**: A completed, failed, killed, interrupted, or indeterminate process
  must not keep the workspace composer in its running state.
- **FR-3**: When a turn terminates naturally, its terminal state must reach an
  already-open workspace without a manual refresh.
- **FR-4**: Initial connection and reconnect must reconcile retained client
  state with a complete current process snapshot, including terminal changes
  whose incremental notification was missed.
- **FR-5**: Final assistant output must not by itself be treated as proof of
  successful process completion.
- **FR-6**: If final output is visible but ordinary terminal evidence is lost,
  the system must use bounded, owner-specific liveness reconciliation and
  eventually record a truthful non-running outcome when activity can no longer
  be proven.
- **FR-7**: Local and cluster-worker execution paths must preserve their
  existing evidence and work-preservation guarantees while converging
  user-visible process state.
- **FR-8**: Active coding-agent, setup, cleanup, and archive processes must
  remain cancellable.
- **FR-9**: Dropped processes, unrelated sessions, queued follow-ups, pending
  approvals, and session switches must not create false running or idle states.
- **FR-10**: Automated regression coverage must reproduce the owning stale
  lifecycle sequence and prove convergence from running to terminal state.

## Out of Scope

- Changes to services other than Vibe Kanban.
- Redesigning the composer or conversation layout.
- Inferring successful completion from assistant message text.
- Broad changes to scheduling, worker affinity, or executor protocols unrelated
  to the stale running state.

## Acceptance Criteria

- [ ] A naturally completed turn changes the open composer's action from Stop
  to Send/continue without refreshing.
- [ ] Completed, failed, killed, interrupted, and indeterminate records all
  render as non-running.
- [ ] A positively active supported process continues to render a working Stop
  action.
- [ ] A missed incremental terminal update followed by authoritative
  reconciliation cannot leave Stop spinning indefinitely.
- [ ] The focused regression fails on the pre-fix behavior and passes with the
  implementation.
- [ ] Relevant frontend and backend checks and formatting pass.

## Open Questions

None. See `clarifications.md`.
