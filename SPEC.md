# Technical Spec: Lazy-load workspace chat history

## Summary

Opening a Vibe Kanban workspace currently replays every normalized log entry for
the newest completed execution processes needed to cross a ten-entry threshold,
then automatically opens more WebSockets and downloads every remaining process
in fifty-entry batches. A single large process is never bounded, and the idle
background loop eventually reconstructs the entire conversation in browser
memory even when the user only reads the latest messages.

Change workspace chat history into a tail-first, demand-driven timeline. The
initial view must contain a bounded recent window, an active execution process
must continue streaming into that window without a second full replay, and older
history must be fetched only when the user reaches the top and requests it.

## Current behavior

- `useConversationHistory` receives the complete execution-process list for the
  selected session.
- Historic processes are fetched newest-first through one
  `/normalized-logs/ws` connection per process. Each connection always replays
  that process from entry zero through `Finished`.
- The initial loader stops only after it crosses `MIN_INITIAL_ENTRIES`; it does
  not trim the oversized process that crossed the threshold.
- `loadRemainingEntriesInBatches` starts immediately afterward and eventually
  downloads all older processes without user intent.
- Running processes use the same endpoint, so historical replay and live
  continuation are coupled. The frontend replaces the process entry array on
  every callback.
- The virtualized renderer limits mounted DOM, but not network transfer,
  normalization work, retained frontend state, or backend replay work.

## Scope

- Vibe Kanban service repository only.
- Local workspace chat and every shared surface that uses the same
  `ConversationList`/history hook (including sidebar and VS Code views).
- Tail-oriented historical retrieval for normalized coding-agent/review logs.
- Demand-driven loading of older pages while preserving scroll position.
- Seamless reconciliation between an initial tail page and subsequent live
  patches for a running process.
- Focused backend and frontend tests plus user-facing loading/error affordances.

## Out of scope

- Changes to `homelab/modules/vibe-kanban-rebuild.nix` or any other service.
- Changing executor log formats or deleting/truncating persisted logs.
- Search, server-side archival, retention policy, or transcript summarization.
- Pagination of script stdout/stderr unless required to keep the conversation
  transport contract internally consistent; script output is not chat history.
- Automatically preloading the complete transcript after the latest window is
  visible.

## Functional requirements

1. Opening an existing workspace displays the most recent coherent chat window
   first, with newest entries in their existing chronological order.
2. Initial historical transfer and retained history are bounded even when the
   newest execution process contains far more entries than the page target.
3. The history contract exposes an opaque continuation cursor (or equivalent)
   and an explicit `has_more` signal. Clients must not infer pagination from
   timestamps, array lengths, or process-local indexes.
4. Repeatedly loading older pages returns every final materialized normalized
   entry exactly once, in deterministic order, with no gaps or overlap at page
   boundaries. The materialized state must incorporate add, replacement, and
   removal patches before paging.
5. Older history is fetched only after explicit user demand (scrolling to the
   top or activating an accessible load-earlier control). Only one older-page
   request may be in flight for a conversation scope.
6. Prepending a page preserves the user's visual anchor; content already on
   screen must not jump when earlier rows are inserted or measured.
7. If a process is running, the client first establishes a bounded snapshot and
   then continues receiving new/replacement patches without losing events in
   the handoff. Live updates append/replace the same process state while older
   pages may be loaded independently.
8. Switching workspace/session scope cancels or ignores every in-flight history
   page and live stream from the prior scope.
9. Reset/deletion/status transitions reconcile loaded pages without reviving
   removed processes, duplicating a completed process, or forcing a full-chat
   reload.
10. The UI distinguishes initial loading, loading an older page, end of history,
    and a recoverable older-page failure. A failed older fetch leaves the
    already loaded tail usable and can be retried.
11. Existing actions that inspect loaded entries (approvals, todos, plan accept,
    edit/reset, and navigation) continue to work for the loaded window. Actions
    targeting unloaded history must first request the necessary page or report
    that the target is not loaded; they must not silently act on the wrong row.

## Non-functional requirements

- Default initial and older page limits are server-enforced and capped; invalid
  or excessive limits are rejected or clamped consistently.
- Cursor values are opaque, validated, scoped to the requested execution log or
  session, and cannot be used to read another workspace's data.
- Backend work per request is proportional to the requested page plus bounded
  cursor/index overhead, rather than the full transcript.
- Existing completed transcripts must be materialized by a cancellable,
  capacity-bounded rollout job before they are advertised as page-ready. An
  interactive page request must not hide a full legacy replay behind a bounded
  response.
- Frontend memory after initial open is proportional to the loaded window plus
  active-stream state, not total conversation length.
- Pagination remains compatible with signed WebSocket/API access used by local
  and remote deployments.
- The implementation adds deterministic tests and no external-service
  dependency.

## Acceptance criteria

- A workspace with thousands of normalized entries opens after transferring a
  bounded recent page; the rest is not fetched while the user remains at the
  bottom.
- The same bounded-open behavior applies to workspaces created before this
  feature once rollout marks their transcript page-ready; preparation is
  observable and does not masquerade as a page request.
- The first visible content is the end of the conversation, not the beginning,
  and an active final turn keeps streaming normally.
- Scrolling to the top loads one older page and keeps the prior first-visible
  message at the same viewport position.
- Loading until `has_more` is false reconstructs the same ordered normalized
  timeline as the legacy full replay, without duplicate stable keys.
- A page-boundary test covers add and replace patches, and a live-handoff test
  proves that an event emitted during snapshot setup is not lost.
- Workspace/session switching prevents stale history or stream events from
  appearing in the new conversation.
- Focused frontend/backend tests, formatting, and relevant repository checks
  pass.

## Risks and mitigations

- **Normalized patches are index based:** paginate a materialized normalized
  state or carry absolute process-local identity so replacement patches remain
  meaningful across pages; test boundaries containing replacements.
- **File streams are naturally forward-only:** add a bounded indexed/tail read
  path instead of reading and normalizing the full log for every page request.
- **Snapshot/live race:** define a replay watermark and subscribe-before-read or
  read-before-subscribe reconciliation protocol; cover the race in tests.
- **Prepend scroll jumps:** capture a semantic row/offset anchor before the
  request and correct after render and measurement.
- **Multiple execution processes:** make the session cursor deterministic across
  process creation time and process-local position, including identical
  timestamps and deleted processes.

## Clarified decisions

- Expose finite historical pages at the session/conversation level and retain a
  revision-aware per-process channel for live continuation.
- Page final materialized normalized entries, defaulting to 100 with a
  server-enforced maximum of 200.
- Keep raw setup, cleanup, and archive script logs on their existing transport.
- Trigger older loading from both top intersection and an accessible
  load/retry control through one single-flight action.

No open questions remain for this requirements deliverable. Follow-up product
implementation must close the storage-atomicity and live-resume detail gates
recorded in the SpecKit analysis before their respective code tasks land.
