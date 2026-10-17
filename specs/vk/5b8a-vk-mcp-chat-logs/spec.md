# Feature Specification: Session chat history through the VK MCP

**Feature dir**: `specs/vk/5b8a-vk-mcp-chat-logs/`
**Status**: Clarified

## Summary

The VK MCP's message tools only return the messages of a session's latest
turn (or of one turn whose id the caller already knows). A session is a
chain of turns, and nothing in the MCP lets a caller read an earlier turn by
time or find where something was said. This feature makes any part of a
session's chat log retrievable through the VK MCP, across every turn, with
filters for date/time, keyword, message role and turn. Results are paged so
the whole log can be walked.

## User Stories

- As an orchestrating agent, I want to read what a worker session said in
  earlier turns, so that a follow-up prompt can build on decisions made before
  the latest turn.
- As a user asking an assistant about past work, I want to search a session's
  chat log for a keyword, so that I can find where a topic (a file, an error,
  a decision) came up without knowing which turn it was in.
- As a user, I want to read what a session said during a given date/time
  window, so that I can review what happened at a particular time.
- As a caller of a large session, I want paged results with a total count, so
  that I can retrieve all of the log in pieces without overflowing a response.

## Functional Requirements

- FR-1: The MCP offers a tool that returns messages from **every** coding-agent
  turn of a given session, not only the latest one.
- FR-2: Results can be filtered by an inclusive date/time window (start, end,
  or both).
- FR-3: Results can be filtered by a keyword (case-insensitive substring of the
  message text). The match considers the full message text even where the
  returned text is shortened, and each match shows the surrounding context.
- FR-4: Results can be filtered by role (user, assistant, system, tool),
  using the same vocabulary as the existing message tools.
- FR-5: Results can be restricted to one turn of the session.
- FR-6: Results are chronological by default, with an option for newest
  first. They are paged with a limit and offset, and the response reports the
  total number of matches and whether more remain.
- FR-7: Each returned message identifies the turn it belongs to (turn id and
  position) and carries a timestamp. If the agent did not record a timestamp
  for a message, the turn's start time is used and labelled as such. No
  invented time is ever shown.
- FR-8: A turn that cannot contain a match for the time window is not read.
- FR-9: The request answers within a bounded time. If the budget runs out
  before every candidate turn is read, the response says the result is
  incomplete and names the turns it did not read.
- FR-10: The tool obeys the same workspace scoping as the existing session
  tools. A session outside the caller's scope is refused.
- FR-11: Invalid input (malformed or inverted time bounds, a turn that does not
  belong to the session) is rejected with a clear message.
- FR-12: The existing message tools keep their current behaviour. Their
  descriptions point callers to the new tool for earlier turns.

## Out of Scope

- Searching across several sessions or a whole workspace in one call.
- Regular-expression or fuzzy matching.
- Returning message text beyond the existing per-message length cap.
- Any change to the web UI.

## Acceptance Criteria

- [ ] A session with three turns returns messages from all three turns when no
      filters are given, oldest first, each tagged with its turn.
- [ ] A keyword search returns only messages containing the keyword in any
      letter case, including a match located past the per-message length cap,
      and each result shows a snippet containing the keyword.
- [ ] A time window excludes messages outside it. Turns entirely outside the
      window are reported as not scanned for cost reasons (scanned count lower
      than the turn count).
- [ ] A message without its own timestamp is included or excluded based on its
      turn's start time.
- [ ] Paging with limit/offset walks the full result with no gaps or overlaps.
      The total and the "more remain" flag are correct, and newest-first order
      reverses the sequence.
- [ ] A turn id from another session, a malformed time, or a start after the end
      is rejected with an explanatory error.
- [ ] When the time budget runs out, the response is marked incomplete and
      lists the turns it did not read.
- [ ] A session outside the MCP's workspace scope is refused.
- [ ] Unit tests cover filtering, paging, snippet and time-window logic.

## Clarified Decisions

See `clarifications.md`.

- One session per call. Workspace-wide search is out of scope.
- The keyword is a single case-insensitive substring.
- Page size defaults to 50 with a maximum of 200. The request budget is 20 s.
- Dropped (soft-deleted) turns are excluded, matching the visible chat.
- The effective time is the entry's own timestamp, else the turn start. Both
  are returned.

## Open Questions

None.
