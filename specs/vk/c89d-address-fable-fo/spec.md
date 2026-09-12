# Feature Specification: Close Stale Execution Follow-up Gaps

**Feature dir**: `specs/vk/c89d-address-fable-fo/`
**Task id**: `vk/c89d-address-fable-fo`
**Status**: Clarified

## Summary

Vibe Kanban must always converge from genuinely active work to a truthful
terminal execution state without a refresh or manual Stop. This feature closes
the follow-up gaps found after PR #226: it makes the composer consume one
activity definition, makes snapshot/live streams lossless and lag-fatal,
reconciles final responses whose worker finalization goes missing, reports
initial connection failure while retaining valid reconnect state, and restores
task-isolated SpecKit history.

## User Stories

- As a user whose agent has finished, I want the composer to return to Send
  automatically so I can continue without manually stopping completed work.
- As a user watching setup, cleanup, or archive work, I want Stop to remain
  available while that work is truly running.
- As a user reconnecting after transport or service disruption, I want the last
  valid workspace state to remain visible and then converge from a complete new
  snapshot.
- As an operator, I want missing stream updates and missing worker finalization
  to become visible, bounded recovery events rather than silent stale state.
- As a maintainer, I want each task's SpecKit artifacts protected from reuse by
  another task.

## Functional Requirements

- **FR-1**: The composer-visible execution provider must use one shared rule for
  deciding whether an attempt is running.
- **FR-2**: A visible execution is cancellable only when its status is exactly
  `running` and its reason is coding agent, setup script, cleanup script, or
  archive script.
- **FR-3**: Completed, failed, killed, interrupted, and indeterminate execution
  statuses must make the composer show Send.
- **FR-4**: Snapshot-plus-live streams must begin observing live mutations
  before capturing their authoritative snapshot and must deliver buffered
  mutations after the snapshot readiness boundary.
- **FR-5**: A lost live mutation or lagged subscriber must invalidate stream
  authority, close the affected connection retryably, and require a complete
  resnapshot.
- **FR-6**: Execution-process, scratch, workspace, browser-session, and message-
  history streams must follow the same handoff and authority-loss contract when
  they combine historical/snapshot state with live changes, or document with
  evidence why the contract does not apply.
- **FR-7**: A final normalized assistant response must start bounded backend
  reconciliation when the expected execution finalization does not arrive, but
  must not alone be treated as proof of successful process exit.
- **FR-8**: Reconciliation must retain `running` only while positive liveness
  evidence exists; after the recovery bound without such evidence it must
  persist the most truthful terminal status without requiring Stop.
- **FR-9**: Work-preservation obligations must be honored before terminalizing
  an execution whose process ownership or exit evidence disappeared.
- **FR-10**: Failure to persist a terminal state must be observable and receive
  a bounded retry or durable later reconciliation.
- **FR-11**: The WebSocket client must distinguish an allocated initial object
  from an authoritative snapshot readiness signal.
- **FR-12**: Repeated failures before the first authoritative snapshot must
  surface a bounded connection error.
- **FR-13**: Reconnect after a valid snapshot must retain that snapshot while
  seeking a replacement, and repeated open-but-never-ready cycles must remain
  backoff-bounded.
- **FR-14**: Relay connections must preserve server diagnostic close code and
  reason through a browser-legal representation while maintaining retry
  behavior.
- **FR-15**: The earlier `vk/5e1e-vk-workspace-cre` record and PR #226's
  `vk/3488-fix-stale-execut` record must both be restored into independently
  owned directories without losing historical artifacts.
- **FR-16**: SpecKit generation must reject a directory owned by another task
  before overwriting any artifact.

## Out of Scope

- Homelab deployment configuration or any service other than Vibe Kanban.
- Treating assistant text as unconditional process-success evidence.
- Redesigning general composer presentation beyond correct Send/Stop and
  cancellation behavior.
- Hiding a patch-sequence gap and continuing from incomplete state.

## Acceptance Criteria

- [ ] Provider/composer boundary tests show Stop for running coding-agent,
  setup, cleanup, and archive executions and Send for every terminal status.
- [ ] A deterministic backend race pauses after subscription and before
  snapshot completion, publishes running then terminal updates, reduces the
  real stream messages, and ends terminal; moving subscription after the query
  makes the test fail.
- [ ] Broadcast lag becomes a stream error and the real execution-process
  WebSocket closes in a manner that triggers reconnect/resnapshot.
- [ ] Every named sibling stream is tested under its applicable shared handoff
  contract or has explicit exemption evidence.
- [ ] A simulated final assistant response with delayed, lost, or interrupted
  finalization converges to a truthful terminal execution through bounded
  reconciliation without manual Stop.
- [ ] Positive worker/process liveness prevents premature terminalization, while
  absence of positive liveness cannot leave a row running indefinitely.
- [ ] The resulting authoritative terminal update returns the composer to Send
  without refresh.
- [ ] Initial connection failures become visible after a bound, while a prior
  Ready snapshot remains rendered during later reconnect failure.
- [ ] Repeated open-before-Ready failures retain increasing bounded backoff, and
  relay error metadata remains diagnosable.
- [ ] Both historical SpecKit tasks occupy their correct isolated directories,
  stale reused-directory files are reconciled, references are correct, and an
  automated collision test rejects cross-task overwrite.
- [ ] Focused tests, formatting, type checks, lint, applicable broad tests, and
  independent Codex review pass.

## Open Questions

Resolved in `clarifications.md`: owner-specific positive liveness, a testable
45-second reconciliation bound, `indeterminate` as the unknown-evidence
fallback, and consumer-facing relay close metadata separated from the
browser-legal underlying transport close.
