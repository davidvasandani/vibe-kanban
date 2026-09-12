# Feature Specification: Vibe Kanban Soft Restarts

**Feature dir**: `specs/vk/9632-vk-soft-restarts/`
**Status**: Implemented

## Summary

The deployed Vibe Kanban cluster must restart its coordinator application without terminating worker-owned coding agents and must never automatically replace a worker while that stable owner has active executions. During the coordinator outage the browser keeps its last good workspace state visible, reports that it is reconnecting, and resumes its streams automatically.

## User Stories

### US-1 — Agent survives coordinator deploy (P1)

As a user with a cluster-worker agent running, I want a Vibe Kanban coordinator update to leave that exact run alive so I do not lose context or work.

**Independent test**: Dispatch a worker execution, stop polling it for the duration of a simulated coordinator restart, then verify it completes with ordered output produced on both sides of the gap.

### US-2 — Worker update waits for active work (P1)

As a user, I want worker release activation to wait until its coding agents are idle so an unattended deployment cannot kill my work.

**Independent test**: Close worker admission, prove new dispatch is refused retryably, prove an existing idempotent dispatch is still recognized, and verify drain safety is false until active executions reach zero.

### US-3 — UI visibly reconnects (P1)

As a user watching a workspace during a coordinator restart, I want current content to remain visible with a temporary reconnect indication.

**Independent test**: Disconnect an initialized workspace JSON-patch stream, verify its last data remains rendered and initialized through retry, then verify connection state drives an accessible status banner that clears on recovery.

### US-4 — Deployment fails safe (P2)

As an operator, I want uncertain, old, or busy worker state to defer activation rather than guess that it is safe.

**Independent test**: Verify workers without the drain contract and workers with nonzero owned work are reported as deferred without changing their active release; only an acknowledged, empty drain proceeds to restart and health gating.

## Functional Requirements

- **FR-1**: Coordinator startup/restart MUST treat worker-owned running rows as non-orphans and reconcile from worker evidence.
- **FR-2**: A coordinator connection or polling gap MUST NOT terminate a worker-owned execution.
- **FR-3**: Worker release activation MUST first close admission for new coding-agent executions.
- **FR-4**: Closing admission MUST be acknowledged by worker-owned state before deployment inspects active work.
- **FR-5**: A drain MUST preserve idempotent retries for executions already accepted while refusing genuinely new dispatch with a retryable response.
- **FR-6**: Worker health MUST report active execution count, admission-draining state, and a `drain_safe` decision derived from those authoritative owner records.
- **FR-7**: `drain_safe` MUST be true only after admission is closed and the active execution count is zero.
- **FR-8**: The admission-drain marker MUST survive the worker process handoff so the candidate cannot accept work before its health gate completes.
- **FR-9**: After a successful candidate health gate, admission MUST reopen; rollback MUST restore the previous release and reopen admission.
- **FR-10**: A worker lacking the race-free drain contract MUST be deferred with an actionable one-time manual activation message.
- **FR-11**: A busy or indeterminate worker MUST remain on its current release and be retried by the existing distribution timer without turning a healthy coordinator deployment into a failure.
- **FR-12**: The browser MUST retain initialized workspace stream data during same-endpoint reconnect attempts.
- **FR-13**: Reconnect MUST use bounded exponential backoff with jitter and reset after success.
- **FR-14**: After initial workspace load, loss of workspace streams MUST display an accessible, non-destructive reconnect status over the still-rendered application.
- **FR-15**: The reconnect status MUST clear when both workspace streams reconnect.
- **FR-16**: Existing immutable release selection, public health checks, matching static rollback, worker request signing, ordered replay, and hard-shutdown behavior MUST remain intact.

## Out of Scope

- Preserving coordinator-local ordinary coding agents in standalone/legacy placement. The deployed cluster's stable-owner boundary is the worker; standalone hot replacement requires moving local execution behind that same boundary.
- Preserving a worker-owned process across a worker binary replacement; this release defers that replacement until the worker is idle.
- Host reboot, kernel failure, or live migration between workers.
- PTY preservation and replay after a coordinator/browser disconnect; terminal journaling is separate work.
- Changes to any service other than Vibe Kanban and its governing Nix deployment module.

## Acceptance Criteria

- [x] A worker execution continues and emits ordered pre/post-gap output without coordinator polling.
- [x] Drain rejects new work, accepts same-digest idempotent retry, and becomes safe only with zero executions.
- [x] Worker `/health` publishes explicit drain evidence.
- [x] Worker distribution defers old/busy workers, persists drain across restart, resumes after health, and retains rollback.
- [x] Initialized workspace data remains rendered across an unexpected WebSocket closure and retry.
- [x] The reconnect status is hidden during first load, shown after a post-load disconnect, and cleared after recovery.
- [ ] Final formatting, checks, independent review, and knowledge-base update pass.

## Open Questions

None. The standalone stable-owner extension and terminal reattachment are explicitly separate follow-up scope, not hidden ambiguity in this deployment feature.
