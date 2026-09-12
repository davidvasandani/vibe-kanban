# Feature Specification: Authoritative Execution Status Reconciliation

**Feature dir**: `specs/vk/3488-fix-stale-execut/`
**Task id**: `vk/3488-fix-stale-execut`
**Status**: Clarified

## Summary

Ensure the workspace chat composer derives Send versus Stop from the latest
authoritative execution state. Initial connection, reconnect, missed events,
interrupted executions, and service restarts must converge to a terminal state
promptly instead of leaving a stale local running flag and indefinite spinner.

## User Stories

- As a workspace user, I want the composer to return to Send when an execution
  finishes so I can start the next turn.
- As a user reconnecting after a network or service interruption, I want the
  workspace to recover current process state without a manual refresh.
- As a user with an active execution, I want Stop to remain visible and
  cancellable until authoritative state becomes terminal.
- As an operator, I want shutdown and recovery to classify abandoned process
  records truthfully so no client can remain permanently running.

## Functional Requirements

- FR-1: The composer must show Stop only when the latest authoritative
  execution state is active.
- FR-2: Completed, failed, killed, interrupted, and applicable indeterminate
  terminal statuses must clear the running composer state.
- FR-3: An initial execution-process connection must hydrate client state from
  a complete current server snapshot before applying incremental updates.
- FR-4: Every reconnect must reconcile local process state with a complete
  current server snapshot, including terminal changes whose events were missed.
- FR-5: Snapshot replacement must remove or terminalize locally cached active
  executions that are no longer active in authoritative state.
- FR-6: Incremental execution updates received after hydration must continue to
  update the composer without regressing newer state.
- FR-7: Service shutdown and startup recovery must not leave an execution
  classified as running when the system lacks positive evidence that its agent
  process remains active.
- FR-8: Interrupted or failed cancellation and transport loss must converge to
  a truthful terminal or indeterminate classification rather than an unbounded
  running state.
- FR-9: Active executions must retain Stop behavior and remain cancellable.
- FR-10: The fix must preserve workspace/session selection semantics and avoid
  treating unrelated executions as the current session's activity.
- FR-11: Automated regression coverage must reproduce a missed terminal event
  followed by reconnect and verify convergence from Stop to Send.
- FR-12: Focused coverage must verify both terminal-state clearing and active
  execution cancellation behavior.

## Out of Scope

- Changes to services other than Vibe Kanban.
- Redesigning the chat composer or execution history presentation.
- Treating a disconnect alone as proof of successful completion.
- Changes to worker affinity, scheduling, or executor protocols beyond what is
  necessary to reconcile existing execution lifecycle state.

## Acceptance Criteria

- [ ] An automated regression test first reproduces a stale Stop/spinner after
  a terminal event is missed.
- [ ] Reconnecting replaces stale active client state with the latest terminal
  server snapshot and the composer renders Send.
- [ ] Restart or interrupted-execution coverage proves the composer cannot
  remain permanently running once backend recovery has classified the process.
- [ ] Completed, failed, killed, interrupted, and applicable indeterminate
  terminal states consistently render the composer idle.
- [ ] A positively active execution still renders Stop and its cancellation
  action remains available.
- [ ] Focused frontend and/or backend tests, formatting, and repository checks
  pass.

## Open Questions

None. See `clarifications.md` for the resolved decisions.
