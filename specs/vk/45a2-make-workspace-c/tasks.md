# Tasks: Workspace chat loads fast when every WebSocket handshake is expensive

**Plan**: `./plan.md`. `[P]` marks a task that touches files no other task in
the same layer touches. All paths are relative to the repo root.

## Layer 0: baseline

- [x] T001 Record the before measurement with the Playwright mobile probe
  (`/tmp/probe/probe.js`). Store the summary in `research.md`. Files:
  `specs/vk/45a2-make-workspace-c/research.md`

## Layer 1: server snapshot sources (FR-1, FR-2, FR-5)

- [x] T010 Add `LogSnapshot`, `normalized_log_snapshot_from_sources` and
  `raw_log_snapshot_from_sources`. Add the trait methods
  `normalized_log_snapshot` and `raw_log_snapshot`. Unit tests:
  - a live store never opens the fallback (pending stream under a timeout);
  - the settled stream materializes and is complete;
  - a missing `Finished` or a running status is incomplete;
  - a `None` stream gives an empty snapshot;
  - raw ordering, live and stored.

  Files: `crates/services/src/services/container.rs`

## Layer 2: routes and types (depends on T010)

- [x] T020 Add `ExecutionProcessLogSnapshot` and the handlers for
  `GET /{id}/normalized-logs` and `GET /{id}/raw-logs`. Each handler drains
  in `tokio::spawn`. Register the routes. Add a test for the wire mapping.
  Files: `crates/server/src/routes/execution_processes.rs`
- [x] T021 Register the type and regenerate. Files:
  `crates/server/src/bin/generate_types.rs`, `shared/types.ts` (generated)

## Layer 3: client building blocks (independent of each other)

- [x] T030 [P] Add the HTTP history loader (`fetchProcessLogSnapshot` and
  `loadHistoricProcessEntries`) and `HISTORY_HTTP_DEADLINE_MS`. Tests: success,
  timeout, error status, `success: false`, abort, malformed JSON,
  finished-during-load fallback, scripts using `raw-logs`, settle-once.
  Files:
  - `packages/web-core/src/shared/hooks/useConversationHistory/fetchHistoricEntries.ts`
  - `…/fetchHistoricEntries.test.ts`
  - `…/constants.ts`
- [x] T031 [P] Add the settled-entries cache with in-flight dedup. Tests: one
  request per process across the initial window and "load earlier"; failures
  are not cached; incomplete results are not cached. Files:
  `packages/web-core/src/features/workspace-chat/model/conversation-history-paging.ts`,
  `…/conversation-history-paging.test.ts`
- [x] T032 [P] Add the shared JSON-patch stream registry. Tests: one socket for
  N subscribers; the socket closes after the linger; a resubscribe inside the
  linger reuses it; reconnect keeps the snapshot; `finished` and a clean close
  do not reconnect; host scope is part of the key. Files:
  `packages/web-core/src/shared/lib/sharedJsonPatchStream.ts`,
  `…/sharedJsonPatchStream.test.ts`
- [x] T033 [P] Add the chat-history-ready store. Files:
  `packages/web-core/src/shared/stores/useChatHistoryReadyStore.ts`
- [x] T034 [P] Canonicalize the discovered-options URL when a session is set.
  Test it. Files: `packages/web-core/src/shared/lib/api.ts`, plus a test next to
  it

## Layer 4: wiring (depends on layer 3)

- [x] T040 Make `useJsonPatchWsStream` a `useSyncExternalStore` adapter over
  T032. Keep the reconnect test green, and reset the registry in it. Files:
  `packages/web-core/src/shared/hooks/useJsonPatchWsStream.ts`,
  `…/useJsonPatchWsStream.reconnect.test.tsx`
- [x] T041 Make `useExecutionProcesses` always request
  `show_soft_deleted=true` and filter `dropped` on the client. Test that
  plain and soft-deleted consumers share one socket. Files:
  `packages/web-core/src/shared/hooks/useExecutionProcesses.ts`,
  `…/useExecutionProcesses.test.ts`, or a new `.tsx` test (depends on T040)
- [x] T042 Wire `useConversationHistory` to the HTTP loader (T030) and the
  per-scope cache (T031). The running → finished reload bypasses the cache.
  Mark the chat history settled (T033). Files:
  `packages/web-core/src/features/workspace-chat/model/hooks/useConversationHistory.ts`,
  `packages/web-core/src/features/workspace-chat/ui/ConversationListContainer.tsx`
  if the workspace id must be passed in
- [x] T043 Gate the mobile diff stream in `WorkspaceProvider` on the active
  tab or the settled chat (T033). Files:
  `packages/web-core/src/shared/providers/WorkspaceProvider.tsx`
- [x] T044 [P] Mount the mobile preview and browser panels on first visit.
  Files: `packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx`

## Layer 5: verification and documentation

- [x] T050 Run `pnpm run generate-types:check`, `pnpm run check`,
  `pnpm run lint`, `cargo test --workspace`, the web-core vitest suite and
  `pnpm run format`.
- [x] T051 Measure after the change with the probe (no added latency, and
  150 ms RTT) against a local build or the coordinator after deploy. Record
  it in `research.md`.
- [x] T052 [P] Update the wiki with the handshake-cost rule, the HTTP
  settlement rules, the numbers and the task id. Files:
  `wiki/awaited-stream-settlement.md`, `wiki/INDEX.md`
