# Feature Specification: Background Workspace Creation

**Feature dir**: `specs/vk/5e1e-vk-workspace-cre/`
**Task id**: `vk/5e1e-vk-workspace-cre`
**Status**: Clarified

## Summary

Accept workspace creation as background lifecycle work so a user can navigate away immediately without cancelling a slow creation. The workspace remains discoverable while it is being prepared, transitions to its normal running experience when ready, and exposes a useful terminal failure if creation cannot complete.

## User Stories

- As a user creating a workspace, I want the request to be accepted quickly so I am not trapped on a form for ten seconds or more.
- As a user who navigates elsewhere after submitting, I want creation and the first agent turn to continue without the create page remaining open.
- As a user returning to the new workspace, I want to see whether it is still being created, ready, or failed.
- As an operator, I want accepted creation work to have one observable identity and an actionable outcome so retries and restarts do not create duplicate agents or indefinite pending state.

## Functional Requirements

- FR-1: The system must reject an empty prompt, an empty repository selection, or contradictory placement choices before accepting workspace creation.
- FR-2: On acceptance, the system must assign and return a durable workspace identity without waiting for repository/worktree materialization or initial execution startup.
- FR-3: All work after acceptance must continue independently of the originating HTTP connection and frontend route.
- FR-4: The accepted operation must preserve the submitted workspace name, repository targets, linked issue, attachments, executor configuration, prompt, and placement intent.
- FR-5: The system must expose an accepted workspace as creating until background work reaches a terminal outcome.
- FR-6: Successful background work must create exactly one initial execution and transition the workspace into the existing ready/running experience.
- FR-7: Failed background work must persist a safe, actionable failure associated with the workspace and must not remain creating indefinitely.
- FR-8: Repeated delivery, retry, or restart recovery for one accepted operation must not create duplicate repository associations, placement operations, or initial executions.
- FR-9: A coordinator restart after acceptance must reconcile unfinished creation from persisted evidence by safely continuing it or recording a truthful terminal/indeterminate outcome.
- FR-10: The frontend must navigate to the accepted workspace as soon as acceptance succeeds and render creation status from authoritative server state rather than mutation-local state.
- FR-11: Completion and failure must become visible through the product's normal refresh/event mechanisms even when the initiating create component has unmounted.
- FR-12: Current linked-issue attachment import, project-context composition, analytics, worker scheduling, coordinator-local placement, and execution configuration behavior must be preserved.

## Out of Scope

- Changes to services outside Vibe Kanban.
- A general-purpose user-configurable job system.
- Redesigning the create form or workspace navigation.
- Automatically retrying arbitrary non-idempotent external failures without bounded policy and durable evidence.

## Acceptance Criteria

- [ ] With workspace materialization deliberately delayed beyond ten seconds, the create endpoint returns an accepted workspace before the delay completes.
- [ ] Cancelling the HTTP client or navigating away after acceptance does not prevent the workspace and initial execution from being created.
- [ ] Opening the accepted workspace during preparation shows a creation-in-progress state sourced from the server.
- [ ] On completion, the same workspace shows its initial session/execution without a second create request.
- [ ] A forced background failure produces a persisted visible failure and no indefinite spinner.
- [ ] Restart/replay coverage proves one accepted workspace produces at most one initial execution.
- [ ] Existing automatic, coordinator, and explicit-worker placement cases retain their semantics.
- [ ] Backend and frontend focused tests, generated-type checks, formatting, and independent review pass.

## Open Questions

None. See `clarifications.md` for the resolved decisions.
