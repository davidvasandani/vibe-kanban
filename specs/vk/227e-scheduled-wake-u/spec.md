# Feature Specification: Scheduled wakeup chat refresh

Feature dir: specs/vk/227e-scheduled-wake-u/
Status: Specified

## Summary
Keep an open conversation current when a scheduled wakeup resumes its session, including after the browser reconnects following the wakeup.

## User Stories
- As a user leaving a chat open, I see the wakeup prompt and response without reloading.
- As a returning user, I see turns completed while disconnected and retain access to earlier history.

## Functional Requirements
- FR-1: New running turns stream into the selected chat automatically.
- FR-2: New completed turns discovered after initial history load appear automatically even if their running state was never observed.
- FR-3: Preserve bounded history loading, stable entry identity, session isolation and removal semantics.
- FR-4: Late asynchronous results from a prior session cannot populate the current session.

## Out of Scope
Scheduler changes, timing changes, other services, deployment changes.

## Acceptance Criteria
- Open chat receives an external running turn and then its settled output.
- A completed follow-up first discovered in a later process snapshot appears without reload or loading older history.
- Existing older unloaded turns remain paginated, and stale results cannot resurrect removed turns or cross sessions.

## Open Questions
None; externally triggered normal follow-up turns are the scope.

## Clarification (/speckit.clarify)
A wakeup is an ordinary session follow-up. Refresh means reconciling its prompt
and output into the existing timeline, including turns first observed terminal
after reconnect. Older unloaded history must remain lazy. No user input is
needed to resolve these boundaries; they follow the existing implementation.
