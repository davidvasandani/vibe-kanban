# Feature Specification: Workspace Server Affinity and Migration

**Feature dir**: `specs/vk/9a64-vk-workspace-aff/`  
**Status**: Draft

## Summary

Show the execution server associated with every workspace and let an operator change that server safely. Stopped workspaces can change affinity directly. Running workspaces require explicit confirmation and one managed transition that stops the current task, changes affinity, and continues the task on the selected server.

## User Stories

- As an operator scanning the workspace drawer, I want to see each workspace's server so that I can understand where work is running without opening it.
- As an operator viewing a workspace, I want to inspect and change its affinity from the right drawer so that placement is visible and controllable in context.
- As an operator changing a stopped workspace, I want reassignment to happen directly so that routine placement changes are quick.
- As an operator changing a running workspace, I want a clear confirmation of the interruption and restart so that I do not stop active work accidentally.
- As an operator after a partial failure, I want to know whether the task stopped, affinity changed, or continuation failed so that I can recover without duplicating work.

## Functional Requirements

- **FR-1:** Every workspace row in the primary left drawer must display a compact, human-readable affinity label.
- **FR-2:** The label must distinguish a resolved worker hostname, coordinator/local placement, automatic placement that is not yet resolved, and an unassigned/indeterminate placement.
- **FR-3:** Drawer affinity data must be obtained without a separate network request or live subscription for every rendered workspace.
- **FR-4:** The workspace right drawer must include a collapsible **Server Affinity** section whose summary or body communicates the current resolved server.
- **FR-5:** The expanded section must provide a **Run on** selector with automatic placement and known execution servers.
- **FR-6:** Servers that cannot accept new work must be identifiable and unavailable as new targets; the current affinity remains identifiable even if its server later becomes unavailable.
- **FR-7:** Choosing the effective current affinity must perform no lifecycle change.
- **FR-8:** When no workspace task is running, selecting a different affinity must validate and persist the choice without asking for restart confirmation.
- **FR-9:** When a workspace task is running, selecting a different affinity must first show a confirmation dialog describing that the current task will stop, affinity will change, and the task will start again.
- **FR-10:** Canceling the confirmation must leave execution and affinity unchanged.
- **FR-11:** Confirming must invoke one coordinator-owned transition rather than require the client to sequence independent stop, affinity, and follow-up actions.
- **FR-12:** The transition must re-check task liveness and target eligibility, stop the current task using established lifecycle semantics, persist the new affinity, and create no more than one continuation execution.
- **FR-13:** The continuation must use the session and execution configuration associated with the task being stopped and a product-owned prompt that tells the agent to inspect existing context and resume unfinished work after migration.
- **FR-14:** Duplicate confirmations, request retries, and lost responses must not create duplicate continuation executions.
- **FR-15:** If stopping fails or cannot be proven, affinity must remain unchanged and no continuation may start.
- **FR-16:** If affinity changes but continuation creation fails, the workspace must remain stopped on the new affinity and the operator must receive a precise recoverable outcome.
- **FR-17:** After success or partial success, all visible affinity and execution indicators must converge on the durable server state without a page reload.
- **FR-18:** Unknown or newly ineligible targets must be rejected by the coordinator even if they appeared selectable in stale client data.
- **FR-19:** Affinity controls and status text must expose loading, pending, success, and actionable error states and prevent duplicate submission.
- **FR-20:** All new operator-facing text must be localized in every supported locale.
- **FR-21:** Existing worker registration, scheduler scoring, workspace filesystem layout, Git worktree ownership, and unrelated services must remain unchanged.
- **FR-22:** Affinity changes must be blocked while a dev server or background helper is running. The error must name the persistent process category and direct the operator to stop it; this feature does not guess how to recreate arbitrary persistent commands on another server.
- **FR-23:** Choosing automatic placement on a stopped workspace must immediately run the scheduler using the latest coding-agent executor profile and persist the selected server with no requested-worker constraint. It must not defer a misleading affinity change until an unknown future execution.
- **FR-24:** The coordinator/local server is not an explicit clustered-mode target because it has no worker identity or worker execution endpoint. Non-cluster deployments display local placement and do not offer a migration selector; clustered deployments offer automatic placement and registered workers.
- **FR-25:** A confirmed migration applies only when exactly one coding-agent execution is running. It stops and continues that execution in its owning session with its persisted executor profile. More than one running coding-agent execution is an invariant violation and must return a conflict without changing state.

## Out of Scope

- Moving or reconfiguring any service other than Vibe Kanban.
- Redesigning worker registration, health leases, draining controls, scheduler scoring, or capacity management.
- Automatically evacuating every workspace from a draining or offline worker.
- Changing shared workspace filesystem or repository topology.
- User-editable migration prompts.
- Migrating a live operating-system process between servers.

## Acceptance Criteria

- [ ] Active, attention, idle, and archived workspace rows display the correct server-affinity label without per-row placement requests.
- [ ] The right drawer shows a persisted Server Affinity accordion with current placement and a Run on selector.
- [ ] Automatic placement and every known worker appear; ineligible new targets are disabled and explained.
- [ ] A stopped workspace can change to automatic or an eligible server without a confirmation dialog, and both drawers update.
- [ ] A running workspace always receives the stop/migrate/restart confirmation before any state changes.
- [ ] Canceling performs no stop, affinity update, or continuation.
- [ ] Confirming produces exactly one coordinator-managed continuation on the selected affinity.
- [ ] Stop failure leaves affinity unchanged; restart failure clearly reports that the workspace is stopped on the new affinity.
- [ ] Stale, unknown, offline, draining, and unhealthy target choices fail safely and actionably.
- [ ] Concurrent/retried migration requests create at most one continuation.
- [ ] Focused backend and frontend tests cover labels, selections, confirmation, success, partial failures, and idempotency.
- [ ] Generated contracts, formatting, type checks, lint, and relevant tests pass.

## Open Questions

None.
