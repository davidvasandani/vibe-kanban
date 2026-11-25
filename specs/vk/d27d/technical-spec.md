# Feature specification for durable session recovery

**Feature directory** `specs/vk/d27d/`

**Status** Draft

## Summary

Vibe Kanban recovers eligible workspace work in the original session when a
worker refuses a dispatch or the coordinator restarts. Recovery runs on the
server without an open browser. Operators can see what VK checked, what it
resumed, and what needs their attention.

## User stories

- As a workspace user, I want VK to continue eligible work after a transient
  refusal so I can keep working in the same session with the original action.
- As an operator, I want a restarted coordinator to pick up surviving work and
  flag uncertain outcomes so I can avoid repeating side effects.
- As a workspace user, I want stop and pause to take precedence over recovery so
  VK respects my decision while retries are pending.

## Functional requirements

- **FR-1** VK must persist the original session, workspace, operation identity,
  action inputs, attempt count, last confirmed step and execution owner before
  dispatch. Recovery must preserve that identity and use the existing execution
  owner. Only one recovery attempt may claim a pending continuation.
- **FR-2** VK must classify failures using typed errors, exit status and bounded
  diagnostics. A successful operation that prints an npm upgrade notice
  requires no recovery. A failed operation that prints the same notice follows
  the ordinary failure classifier.
- **FR-3** VK may recover only predefined, workspace-scoped operations. For a
  definitively refused dispatch, VK must verify worker readiness before
  continuing. For a transient failure during a read-only workspace check, VK
  must repeat the check at most once and continue the pending step only if the
  check passes. Record the original failure, retry and verification in session
  history before dispatching the pending step.
- **FR-4** On startup, VK must reconcile only active work owned by the
  interrupted coordinator instance, including unfinished recovery attempts.
  Verify executor identity and check durable execution evidence before taking
  ownership. Reattach to surviving executions, preserve confirmed completions
  and resume only from checkpoints known to be safe.
- **FR-5** When side effects are uncertain, VK must request operator review
  before repeating the operation. Unsupported and exhausted failures must keep
  their diagnostics and show a concrete manual next action. Preserve the
  distinction between killed, interrupted and indeterminate outcomes.
- **FR-6** A stop or pause must take precedence over pending recovery. Inactive
  work must stay inactive during startup reconciliation. Replaying events must
  never dispatch work.

## Out of scope

- Arbitrary remediation commands, npm upgrades and recovery through a new
  workspace or session.
- Changes to automatic continuation after a transient provider error in an
  agent turn.

## Acceptance criteria

- [ ] On a dedicated VK instance with no browser connected, interrupt the
  coordinator during execution and interrupt its replacement during
  reconciliation. Verify that surviving work is reattached, completed effects
  occur once, safe checkpoints continue once, and uncertain effects require
  review. Inspect the persisted session history after the second restart.
- [ ] Definitively refuse an eligible dispatch before it starts. Verify that VK
  records the refusal and readiness result, then dispatches once with the
  original session, workspace and action inputs.
- [ ] Fail a read-only check with a transient error. Verify that VK retries the
  check once and continues the pending step once if the check passes. If it
  fails again, verify that VK preserves the diagnostics and shows a manual
  next action.
- [ ] Replay the same events and verify that the dispatch and side-effect counts
  stay the same. Stop or pause work before recovery dispatch and verify that it
  remains inactive across restart.
- [ ] Run successful and failing operations that print the same npm upgrade
  announcement. Verify that the successful operation completes normally and
  the failing operation follows the ordinary failure classifier.

## Clarifications
The supported dispatch failure is an explicit worker refusal before execution starts. VK may make one replacement dispatch after verifying readiness. A timeout or dropped reply requires reconciliation by execution identity before any replacement. The readiness probe itself is the read-only workspace check and may retry once.
