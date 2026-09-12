# Feature Specification: Lazy-load workspace chat history

**Feature dir**: `specs/vk/65ab-lazy-load-vk-wor/`
**Status**: Draft

## Summary

Open an existing workspace conversation from a bounded recent window instead of
reconstructing the complete transcript. Users can request earlier history while
the newest active turn continues streaming, reducing startup time, server work,
network traffic, and browser memory for long conversations.

## User stories

- As a user reopening a long workspace, I want to see its latest messages
  quickly so I can resume work without waiting for the full transcript.
- As a user reading context, I want earlier messages to load when I reach the
  top so I can still recover the complete conversation when needed.
- As a user watching an active agent, I want new output to continue arriving
  while older history remains unloaded or is being fetched.
- As a keyboard or assistive-technology user, I want an explicit load-earlier
  action and useful loading/error state so history access does not depend only
  on pointer scrolling.

## Functional requirements

- **FR-1**: The system must initially return and display a bounded window from
  the end of an existing workspace conversation in chronological display order.
- **FR-2**: The initial bound must hold even when one execution process contains
  more history than the configured window.
- **FR-3**: The system must indicate whether earlier history exists and provide
  opaque continuation state for retrieving it.
- **FR-4**: Each earlier-history request must be bounded and must return the
  immediately preceding window in deterministic order.
- **FR-4a**: Page size is a maximum, not an expected exact count. Clients must
  use only the returned continuation state and `has_more` to decide whether
  earlier history exists.
- **FR-5**: Following continuation state until exhaustion must reproduce every
  normalized conversation entry exactly once with the same final content and
  ordering as a complete replay.
- **FR-6**: The client must not request earlier pages merely because the recent
  window finished loading; it requests one only in response to user demand.
- **FR-7**: At most one earlier-history request may be active for a conversation
  scope, and repeated top-reached signals during that request must be coalesced.
- **FR-8**: Prepending earlier history must preserve the visible reading anchor.
- **FR-9**: A running execution must continue delivering new and replacement
  entries after the bounded recent snapshot without an event-loss or duplicate
  interval.
- **FR-10**: Loading earlier history must not pause, replace, or restart the
  active live stream.
- **FR-11**: Changing workspace or session must cancel or ignore all historical
  and live results owned by the previous scope.
- **FR-12**: Process completion, reset, and deletion must reconcile the loaded
  window without duplicating completed entries or reviving removed entries.
- **FR-13**: The UI must distinguish initial loading, older-page loading, no
  earlier history, and recoverable older-page failure.
- **FR-14**: A failed earlier-page request must preserve the usable recent
  window and offer retry.
- **FR-15**: Continuation state and page bounds must be validated and scoped to
  the authorized conversation; malformed, expired, or cross-scope state must
  fail safely.
- **FR-16**: Abandoned initial/page requests must stop associated backend work
  promptly.
- **FR-16a**: Conversations created before normalized materialization exists
  must be prepared by observable, cancellable, capacity-bounded rollout work
  before the page API reports them ready. Interactive page requests must not
  perform a hidden complete raw-log replay.
- **FR-17**: Loaded-entry actions (including approvals, todos, planning,
  edit/reset, and navigation) must retain their existing semantics; an action
  must not silently resolve an unloaded target to a different loaded row.

## Out of scope

- Homelab deployment or hosting changes.
- Other services in the shared homelab repository.
- Log deletion, retention, archival, search, or transcript summarization.
- Changing agent/vendor event formats.
- Pagination of raw setup, cleanup, and archive script output; those diagnostics
  remain on the existing raw-log stream.

## Acceptance criteria

- [ ] Opening a fixture conversation with thousands of entries transfers and
  retains only the configured recent window until the user asks for more.
- [ ] The initial rendered rows are the latest conversation rows in existing
  chronological order.
- [ ] A single execution process larger than the page target still produces a
  bounded initial response.
- [ ] A legacy workspace is served only after page-ready materialization; its
  interactive request does not invoke full historical normalization.
- [ ] Remaining idle at the bottom produces no earlier-history request.
- [ ] Reaching the top loads exactly one older window and retains the same
  first-visible row and offset after prepend.
- [ ] Repeatedly loading earlier windows to exhaustion produces the same final
  normalized timeline as legacy complete replay, including replacement patches,
  without duplicate stable keys.
- [ ] An event emitted at the bounded-snapshot/live-stream boundary appears
  exactly once.
- [ ] An active execution continues updating while an earlier page is in flight.
- [ ] A page failure leaves recent messages interactive and retry succeeds.
- [ ] Switching scope during a slow page/stream prevents any stale row from
  appearing in the new conversation.
- [ ] Malformed and cross-scope continuation state is rejected.
- [ ] Focused backend/frontend tests and repository verification pass.

## Clarified decisions

- Continuation is conversation/session-level; process position remains opaque.
- Pages contain final materialized normalized entries, default to 100, and are
  capped by the server at 200.
- Top intersection and an accessible load/retry control invoke the same
  single-flight action.
- Raw script logs remain on their existing transport.

See `clarifications.md` for rationale. No open questions remain.
