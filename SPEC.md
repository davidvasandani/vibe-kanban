# Technical Specification: Show Sent Chat Messages Without Refreshing

## Objective

Ensure that a successfully submitted follow-up message appears in the active
Vibe Kanban conversation immediately and remains visible as live execution data
arrives, without requiring a browser refresh.

## Problem

The existing-session send path waits for the execution-process WebSocket to
announce the process returned by the successful follow-up request. When that
stream update is delayed or missed, the composer is cleared but the new user
turn is absent from the conversation until a page refresh rebuilds history from
the server. This makes a successful send look lost even though the backend has
accepted it.

## Scope

This change is limited to the Vibe Kanban service repository. No other service
or deployment configuration is changed.

The feature covers follow-up messages sent from an existing session. New
session creation, queued follow-ups, approval responses, and backend execution
semantics remain unchanged unless shared reconciliation logic requires focused
tests to prove they are unaffected.

## Functional Requirements

1. After the follow-up API succeeds, the execution process returned by that
   response must be reconciled into the client-side execution-process state.
2. Conversation history must discover and render that process even if the
   WebSocket notification is delayed, arrives before the HTTP response, or is
   not observed by the current connection.
3. HTTP and WebSocket delivery of the same process must be idempotent: one send
   produces one visible user turn and one live-log subscription.
4. The submitted text must be represented by the server-returned execution
   process rather than by a second, synthetic message record, so server IDs,
   timestamps, executor configuration, and later status transitions remain
   authoritative.
5. The composer and attachments are cleared only after the backend accepts the
   follow-up, preserving the current failure behavior.
6. Session changes must not leak a reconciled process into another session.
7. The existing execution-process stream remains the authority for subsequent
   process updates and removal/reset behavior.

## UX Requirements

- A successfully sent follow-up appears in the conversation during the same UI
  interaction, with no manual reload.
- No duplicate user bubble or visual flicker is introduced when the stream
  later reports the same process.
- Failed sends continue to leave the draft available and surface the existing
  error feedback.

## Technical Direction

- Introduce a narrowly scoped client-side reconciliation path from the
  successful `sessionsApi.followUp` response into the execution-process data
  consumed by conversation history.
- Key reconciliation by execution-process ID and validate the active session
  before accepting it.
- Preserve the WebSocket snapshot and patch behavior; the HTTP response closes
  the creation-notification race but does not replace live status/log streams.
- Prefer a testable pure reducer/helper or provider action over component-local
  duplicated conversation entries.

## Verification

Automated tests must cover:

1. A successful follow-up becomes visible from the API response before any
   WebSocket process patch.
2. A later WebSocket patch/snapshot for that ID does not duplicate the turn.
3. A WebSocket update that wins the race with the API response remains
   idempotent.
4. A process belonging to a different session is rejected or ignored.
5. A failed follow-up does not insert a process and does not clear the draft.
6. Existing live status updates continue to replace the reconciled process.

Run focused frontend tests plus the repository-required formatting and relevant
type/lint checks.

## Out of Scope

- Changes to the homelab deployment module or any other hosted service.
- Changing backend follow-up persistence or executor startup semantics.
- Redesigning conversation history, queued messages, or the composer.
- Fabricating an optimistic process before the server accepts the request.

## Acceptance Criteria

- Reproduction with a delayed or absent execution-process stream notification
  shows the sent follow-up without refreshing.
- Normal stream delivery yields exactly one rendered turn.
- Automated regression coverage exercises both orderings of the HTTP/WebSocket
  race and session isolation.
- Independent Codex review reports no significant findings.
- Reusable knowledge is recorded in the project knowledge base, and the task's
  pull request is merged into the base branch.
