# Feature specification: exclusive session turns

Applied `/speckit.specify`. Feature directory uses the workspace task pointer
`specs/vk/0890-investigate-a-fo` within Vibe Kanban, the service in scope;
the repo-local command's hard-coded browser-scrolling path is stale.

## Summary
Prevent overlapping coding turns from editing one worktree/transcript and prevent
native Claude peer discovery from redirecting messages to unrelated workspaces.

## User stories
- A user can correct a starting or running turn and have that correction wait.
- An operator can trust that a worker cannot run two agents for one session.
- An agent can identify its VK session consistently without guessing peer identity.

## Requirements
FR-1: All coding launch paths enforce session exclusivity before startup awaits.
FR-2: Busy ordinary follow-ups queue; retries that mutate history cannot reset a live turn.
FR-3: Worker admission independently rejects concurrent session jobs and preserves same-execution replay.
FR-4: Unsupported native cross-session peer tools are unavailable in every permission mode, with explicit guidance to use targeted VK delivery.
FR-5: Supported identity is derived from full VK workspace/session UUIDs, stable across turns.
FR-6: Startup failure, cancellation and finalization preserve ownership and allow subsequent legitimate turns.

## Acceptance
- Regression covers a follow-up nine seconds into initial startup and simultaneous admissions.
- Duplicate worker dispatch cannot create a second session process; replay stays idempotent.
- Native peer tools cannot accidentally target another workspace in bypass mode.
- Tests and documentation explain identity and collision reporting.

## Scope
Vibe Kanban only. No CLI version bumps or unrelated service changes.

## Open questions
None requiring user input. Verify native peer isolation support in current launch integration;
use the authorized disable option if no enforceable workspace-scoped interface exists.
