# Feature Specification: Show Sent Chat Messages Without Refreshing

**Feature dir**: `specs/vk/8a08-frontend-not-ref/`
**Status**: Clarified

## Summary

Make every successfully accepted follow-up appear in the active conversation
without a browser refresh. A delayed or missed live execution notification must
not make a sent message appear lost, while duplicate delivery through request
and live-update paths must still produce only one turn.

## User Stories

- As a user, I want a successfully sent follow-up to appear immediately so that
  I know Vibe Kanban accepted it.
- As a user, I want the conversation to remain correct when the live connection
  is delayed or reconnecting so that I do not need to refresh the page.
- As a user moving between sessions, I want each sent message to appear only in
  its owning session.

## Functional Requirements

- FR-1: A successfully accepted follow-up must become visible in its active
  conversation during the same interaction, without requiring a refresh.
- FR-2: The system must use the server-confirmed execution identity and data for
  the visible follow-up.
- FR-3: The follow-up must become visible even when its live creation
  notification is delayed or not observed by the current connection.
- FR-4: Receiving the same execution through both the request response and live
  updates must produce exactly one visible turn.
- FR-5: Later authoritative execution updates must continue to update that same
  turn, including status changes and removal/reset behavior.
- FR-6: A returned execution must be associated only with its owning session and
  must not appear after the user changes to another session.
- FR-7: A failed follow-up request must not add a turn or clear the user's draft
  and attachments.
- FR-8: New-session creation, queued follow-ups, approvals, and backend execution
  behavior must retain their current semantics.

## Out of Scope

- Changes to another service or to Vibe Kanban deployment configuration.
- Fabricating a local message before the backend accepts the follow-up.
- Redesigning conversation history, the composer, or queued-message behavior.
- Changing execution persistence or coding-agent startup behavior.

## Acceptance Criteria

- [ ] With live execution creation delivery delayed, a successful follow-up is
      visible before the delayed live update arrives.
- [ ] With live execution creation delivery absent, the successful follow-up is
      visible without a manual refresh.
- [ ] When request and live delivery occur in either order, the conversation
      contains exactly one turn for the execution.
- [ ] A later live update replaces the matching response-derived state.
- [ ] A response belonging to a different or no-longer-active session does not
      appear in the current conversation.
- [ ] A failed request leaves the draft available and adds no conversation turn.
- [ ] Focused automated regression tests pass for the shared frontend surface.

## Open Questions

No open questions remain.

## Clarifications

- Response-derived execution state remains available until the active session's
  live projection reports the same execution ID or the session scope changes.
  It does not expire on a timer: a successful server response is durable
  creation evidence, and arbitrary timeout expiry would recreate the original
  disappearing-message defect during a prolonged stream outage.
- Once live delivery reports the matching execution, the live value supersedes
  the response-derived value. Subsequent live replacement or removal behavior
  proceeds through the existing authoritative stream path.
