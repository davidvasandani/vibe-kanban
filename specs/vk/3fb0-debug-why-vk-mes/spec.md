# Feature Specification: Message reads return while a turn is still running

**Feature dir**: `specs/vk/3fb0-debug-why-vk-mes/`
**Status**: Draft

## Summary

Reading the messages of an execution that is still running never returns. The
request hangs for as long as the agent keeps working and the caller eventually
times out. This affects the `/messages` HTTP endpoint and both MCP tools built
on it (`list_recent_messages`, `list_all_messages`) — the very tools documented
for checking on a turn *before* nudging it, which is by definition a running
turn. Reads of finished executions are unaffected. We want a message read to
return the conversation so far, promptly, whatever state the execution is in.

## User Stories

- As an orchestrating agent, I want to read what a running session has said so
  far, so that a follow-up prompt responds to the agent's actual output instead
  of guessing from status and exit code.
- As a user watching a live workspace, I want message-backed views to populate
  immediately rather than spinning until the turn ends.
- As a developer, I want a read of a running turn to be distinguishable from a
  read of a finished one, so that I know whether more messages are still coming.

## Functional Requirements

- FR-1: A message read MUST return within a normal request budget for an
  execution in any state, including one still running.
- FR-2: For a running execution the response MUST contain the messages
  normalized so far — not an error, not an empty list, and not a placeholder.
- FR-3: The response MUST carry the execution's authoritative status, so a
  caller can tell a partial (running) read from a settled (finished) one.
- FR-4: Reads of finished executions MUST be unchanged: same messages, same
  ordering, same message identity, same role filtering, same per-message
  truncation, same `has_more` semantics, same recent/all distinction.
- FR-5: The live log stream that the UI subscribes to MUST keep following a
  running execution exactly as it does today. Fixing the read must not convert
  the subscriber into a snapshot.
- FR-6: A read MUST NOT trigger reconstruction work for an execution whose
  messages are already available in memory.
- FR-7: The recent/all selection, role filter, and message-identity scheme MUST
  behave identically for running and finished executions.
- FR-8: When an execution's retained history has been evicted under its size
  cap, the read MUST still return promptly with the messages that survive,
  skipping only the entries that can no longer be reassembled. It MUST NOT
  hang, error, collapse to an empty list because the oldest entry was lost, or
  grow the response shape to report the eviction.
- FR-9: For a running execution, the reported final message MUST be the latest
  assistant text produced so far, or absent if there is none yet. It is a
  progress signal, never evidence that the turn has ended.

## Out of Scope

- The websocket streaming contract and its reconnect/lag behaviour.
- The normalized-log cache and historical re-normalization design, including
  the newest-2,000-normalizable-message bound for legacy cache misses.
- Pagination, cursors, or raising the message limits.
- Incremental/resumable message reads (a read stays one-shot).
- Notifying a caller when new messages arrive after its read.

## Acceptance Criteria

- [ ] Reading messages for an execution that is still running returns the
      buffered conversation, under an assertable deadline, with its status
      reported as running.
- [ ] A regression test covers the still-running read and fails as a timeout —
      not a hang — if the unbounded wait is reintroduced.
- [ ] A running execution that has produced no messages yet returns an empty
      message list promptly, rather than blocking until it produces one.
- [ ] A running execution whose oldest entries were evicted still returns the
      messages that survive, rather than an empty list.
- [ ] A finished execution still reads from its settled stream.
- [ ] Non-conversation patches (repo diffs) stay excluded from message results
      for running executions, as they already are for finished ones.
- [ ] Existing finished-execution tests pass unchanged.
- [ ] `cargo test --workspace`, `pnpm run check`, and `pnpm run lint` pass.

## Resolved Clarifications

All three questions raised at spec time are resolved; see
`clarifications.md` for the reasoning and evidence.

- **C-1 — Byte-capped history is returned silently.** Retained history is capped
  at 100 MB per execution, so eviction during a single turn is a remote edge
  case rather than a routine one. The read returns the retained tail without a
  new truncation signal, matching what the live subscriber already sees. This
  adds no field to the response shape. Folded in as FR-8.
- **C-2 — `final_message` reports the latest assistant text so far.** No
  special case for running executions: it is derived from the same messages the
  response already carries. Callers must not read a non-null `final_message`
  as evidence the turn ended — the status field is the authority for that.
  Folded in as FR-9.
- **C-3 — `has_more` keeps its current meaning only.** It continues to mean
  "more messages matched than the limit allowed through". It is not overloaded
  to mean "this turn may produce more messages"; status already conveys that,
  and overloading it would silently change the meaning for existing callers
  (FR-4 requires finished-read semantics to be unchanged).

## Open Questions

None. All items are resolved above.
