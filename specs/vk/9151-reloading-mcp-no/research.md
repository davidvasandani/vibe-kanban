# Research: Reliable MCP Reload

## Decision: hydrate from the existing status endpoint

The GET endpoint already exposes the coordinator's canonical session state.
Loading it on session entry fixes remount/navigation loss without persistence,
new routes, or backend changes.

## Decision: reconcile `busy`, do not poll it

`busy` is a response projection for a duplicate POST; it is not stored by the
coordinator. Polling only the projection would be semantically wrong, while
stopping at it loses the real pending generation. An immediate GET converts it
back to canonical state.

Alternative rejected: change duplicate POST to return `pending_next_turn`.
That erases useful feedback that a request was already in progress and changes
an established backend contract solely to accommodate browser state loss.

## Decision: isolate lifecycle logic in a feature-local hook

`SessionChatBoxContainer` has broad dependencies and no rendered test harness.
A hook with injected/default API behavior gives the async state machine a narrow
test surface and keeps the container focused on composition. No external state
library or new dependency is needed.

Alternative rejected: add another inline effect and callback. That would leave
request, hydration, polling, session-race guards, and toast transitions spread
across a large component and make the reported lifecycle hard to test.

## Existing backend behavior preserved

- pending generations are session-scoped and generation-based;
- duplicate requests return retryable `busy` without changing stored state;
- success is confirmed from Codex MCP status at the safe execution boundary;
- errors remain allow-listed and secret-safe.
