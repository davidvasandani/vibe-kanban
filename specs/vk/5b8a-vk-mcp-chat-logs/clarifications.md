# Clarifications: session chat history through the VK MCP

No answers were supplied with `/speckit.clarify`. Each open question is
resolved with the default below, chosen to keep the change small (constitution
III) and bounded (LII).

## 1. One session per call, or a whole workspace?

**Decision:** One session per call (`session_id` required).

The request is "the session's latest turn" being the only thing reachable, so
the gap is per session. A workspace-wide scan multiplies the cold-normalization
cost by the number of sessions and needs cross-session paging. Callers can
already `list_sessions` and search each one. A workspace-wide variant can be
added later on top of the same per-session logic.

## 2. Keyword syntax

**Decision:** One case-insensitive substring (trimmed; empty means no filter).

This covers "find where X was said". Multi-term and regex matching add parser
and denial-of-service surface (pathological patterns) for little gain, because
an agent can issue several calls.

## 3. Page size and time budget

**Decision:** Default page 50 messages, maximum 200. Request budget 20 s,
checked before loading each turn.

200 messages × the existing 4000-char cap stays well under typical MCP response
limits. 20 s leaves margin under common 60 s MCP client tool timeouts. The turn
already loading when the budget passes finishes, so its normalized sidecar is
written and the retry is cheaper.

## 4. Dropped (soft-deleted) turns

**Decision:** Excluded, matching `/messages`, which reads with
`show_soft_deleted = false`.

Dropped turns were reverted by the user (retry/reset) and the UI hides them.
Including them would make search results disagree with the visible chat.

## 5. Timestamp semantics (raised during review)

**Decision:** The effective time is the entry's own RFC 3339 timestamp when
present, otherwise the turn's `started_at`. The response carries both
`created_at` (the entry's own value, possibly null) and `turn_started_at`, so
the fallback is visible. A turn's time span for pruning is
`[started_at, completed_at or now]`.

## Remaining questions

None.
