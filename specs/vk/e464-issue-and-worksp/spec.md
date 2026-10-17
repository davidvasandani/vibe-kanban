# Feature Specification: Issue and workspace lifecycle

**Feature dir**: `specs/vk/e464-issue-and-worksp/`
**Status**: Specified

## Summary
Resuming a conversation activates its archived workspace, and reactivating a
workspace reopens its completed issue so the board reflects resumed work.

## User Stories
- As a user, I want posting a workspace message to restore the workspace to my active list.
- As a user, I want reopening a workspace on a Done issue to move the issue to In progress automatically.

## Functional Requirements
- FR-1: Accepting a workspace conversation message, including a queued follow-up, activates an archived workspace.
- FR-2: An archived-to-active workspace transition moves its linked Done issue to that project's In progress status.
- FR-3: Explicit unarchive and message-driven unarchive have the same issue effect.
- FR-4: Other issue statuses, unlinked workspaces, already-active workspace updates and read operations retain their existing semantics.
- FR-5: Resolve status names case-insensitively, consistently with existing project behavior. If the project has no In progress status, preserve the issue status.
- FR-6: Workspace and issue changes within the remote store commit atomically; synchronization failures follow existing observable retry behavior.

## Out of Scope
Issue-level comments without a workspace target, reopening parent issues, schema/status category redesign, other services and deployment changes.

## Acceptance Criteria
- [x] Accepted direct and queued comments reactivate archived workspaces.
- [x] Explicit and synchronized archived-to-active transitions reopen Done issues.
- [x] Repeated active updates, metadata edits and archive requests do not reopen Done issues.
- [x] Non-Done, unlinked, and missing-target-status cases are safe.
- [x] A failed remote mutation cannot commit only one half of the lifecycle transition.

## Open Questions
None; clarify stage records semantics and rationale.

## Clarifications (/speckit.clarify)
- “Commenting on a workspace” means submitting a workspace conversation prompt,
  including queue acceptance, rather than an issue-level discussion comment.
- Active means not archived; it does not require a currently running executor.
- Only Done reopens; Cancelled and custom statuses are preserved.
- Missing In progress follows the existing name-based optional status convention.
