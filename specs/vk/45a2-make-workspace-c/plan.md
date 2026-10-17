# Implementation Plan: Workspace chat loads fast when every WebSocket handshake is expensive

**Spec**: `./spec.md`
**Status**: Draft
**Task**: `vk/45a2-make-workspace-c`

## Technical Context

- Backend: Rust (axum, tokio). The routes live in
  `crates/server/src/routes/execution_processes.rs`. Log sources live in
  `crates/services/src/services/container.rs` (`ContainerService` trait:
  `stream_normalized_logs`, `stream_raw_logs`, `normalized_entries`) and in
  `normalized_log_cache.rs` (`materialize_entries`,
  `materialize_entries_rebased`). Raw logs for finished processes come from
  `services::execution_process::load_raw_log_messages`.
- Frontend: React and TypeScript in `packages/web-core`, tested with Vitest.
  Transport goes through `shared/lib/localApiTransport.ts`
  (`makeLocalApiRequest` and `openLocalApiWebSocket`, both host-scoped).
  remote-web replaces them with relay and WebRTC versions through
  `setLocalApiTransport`.
- Generated types: `crates/server/src/bin/generate_types.rs` →
  `shared/types.ts`, built with `pnpm run generate-types`. `PatchType` is
  already exported.
- Constraint: no new dependencies. The summaries path (#350), NFS and
  Cloudflare are out of scope.

## Architecture & Approach

### A. Server: settled log snapshots (FR-1, FR-2, FR-5)

`crates/services/src/services/container.rs`:

1. `pub struct LogSnapshot { pub entries: Vec<serde_json::Value>, pub complete: bool }`.
2. Free function `normalized_log_snapshot_from_sources(id, live_store, process_running, settled_stream)`:
   - Live store present: snapshot with `indexed_entry_patches_from_history`,
     then `materialize_entries`, falling back to `materialize_entries_rebased`.
     Return `complete: false`. The settled stream is never opened. This is
     the same choice `normalized_entries_from_sources` makes, and XXXIX
     applies.
   - Otherwise drain the finite stream, keeping `/entries/<n>` patches.
     `complete = saw_finished && !process_running`. A stream error or a
     missing `Finished` gives `complete: false`.
   - `None` stream: no logs, so `Some(LogSnapshot { entries: [], complete: !process_running })`.
     The socket sends `finished` immediately in this case, so the HTTP
     response matches it.
3. Free function `raw_log_snapshot_from_sources(live_store, process_running, stored_messages)`:
   stdout and stderr become `PatchType::Stdout/Stderr` values in order. A live
   store gives a history snapshot with `complete: false`.
4. Trait methods `normalized_log_snapshot(&self, id, process_running)` and
   `raw_log_snapshot(&self, id, process_running)` call the free functions with
   `self.get_msg_store_by_id(id)` and `self.stream_normalized_logs(id)` /
   `load_raw_log_messages`. They reuse the sidecar, the lease, the permit and
   the bounded historical normalization unchanged.

`crates/server/src/routes/execution_processes.rs`:

5. `ExecutionProcessLogSnapshot { entries: Vec<JsonValue> (#[ts(type = "Array<PatchType>")]), complete: bool }`.
6. `GET /{id}/normalized-logs` and `GET /{id}/raw-logs` sit under the existing
   `load_execution_process_middleware` and are registered next to the `/ws`
   routes. The handler runs the snapshot inside `tokio::spawn` and awaits the
   `JoinHandle`. Dropping the request therefore does not drop the drain, and a
   cold normalization finishes and writes its sidecar (FR-5). The lease and
   permit still bound it. A join error maps to `ApiError` (500).
7. Register `ExecutionProcessLogSnapshot` in `generate_types.rs`.

### B. Client: HTTP history loader (FR-1 – FR-4)

New file `packages/web-core/src/shared/hooks/useConversationHistory/fetchHistoricEntries.ts`:

- `fetchProcessLogSnapshot(process, { deadlineMs, request })` has an
  `AbortController` deadline and settles once through a `settled` guard. It
  rejects on timeout (`HistoryFetchTimeoutError`), abort, non-2xx,
  `success: false` and malformed JSON. `request` defaults to
  `makeLocalApiRequest`. The URL is `…/raw-logs` for `ScriptRequest`
  processes, otherwise `…/normalized-logs`.
- `loadHistoricProcessEntries(process, { fetchSnapshot, streamFallback })`
  returns `{ entries, complete }`. When the snapshot is incomplete it calls
  `streamFallback`, the existing socket loader with
  `HISTORY_STREAM_IDLE_TIMEOUT_MS`, and returns `complete: true` only once
  `finished` arrives.
- `constants.ts`: `HISTORY_HTTP_DEADLINE_MS = 30_000`.

### C. Client: once per scope (FR-6)

`conversation-history-paging.ts` gets `createSettledEntriesCache()`: a Map of
complete entries per process id, plus in-flight promise dedup. `get(process,
load)` returns a cached value, joins an in-flight request, or starts one, and
stores the result only when it is complete. Failures are neither cached nor
retained in flight.

`useConversationHistory.ts`:
- A `settledCacheRef` is recreated in the scope-reset effect.
- `loadEntriesForHistoricExecutionProcess` → `settledCacheRef.current.get(process, loadHistoricProcessEntries)`.
- The running → finished reload calls `loadHistoricProcessEntries` directly
  (bypassing any cache) and stores the result when it is complete.

### D. Client: shared live streams (FR-7, FR-8, FR-10)

New file `packages/web-core/src/shared/lib/sharedJsonPatchStream.ts`:
- `SharedJsonPatchStream<T>` keeps a snapshot `{data, isConnected, isInitialized, error}`,
  its subscribers, the socket, the retry timer, the attempt count, the
  `finished` flag and a linger timer.
- The logic moves over from `useJsonPatchWsStream.ts`: `getReconnectDelay`,
  `Ready` resets backoff, `finished` is terminal, a clean 1000 close does not
  reconnect, the snapshot is kept across reconnects, and the error is raised
  after 6 failed attempts without `Ready`.
- `acquireSharedJsonPatchStream(key, endpoint, socketOptions, initialData)` uses a
  module registry. The key is the endpoint plus the resolved host scope
  (`getCurrentHostId()` for the `current` scope).
- `STREAM_LINGER_MS = 3_000`.
- `resetSharedJsonPatchStreamsForTests()`.

`useJsonPatchWsStream.ts` becomes a `useSyncExternalStore` adapter over the
shared stream. `getReconnectDelay` stays exported for its existing test.
Options that change data per consumer (`injectInitialEntry`,
`deduplicatePatches`, which have no callers today) make the stream private by
adding a unique suffix to the key.

`useExecutionProcesses.ts` always sends `show_soft_deleted=true` and filters
`!dropped` on the client unless `showSoftDeleted` is set. This matches the
server's filter in `events/streams.rs`.

In `shared/lib/api.ts`, `getDiscoveredOptionsStreamUrl` omits `workspace_id`
and `repo_id` when `session_id` is set, matching the server's resolution in
`discover_executor_options`.

### E. Client: the chat goes first on mobile (FR-9)

- New store `shared/stores/useChatHistoryReadyStore.ts` (zustand, which is
  already a dependency). It holds `settledWorkspaceIds: Set` with
  `markSettled(id)`.
- `useConversationHistory` calls `markSettled(workspaceId)` on its first
  non-loading `'initial'` emit. The workspace id comes from the existing
  `useWorkspaceContext`, or is passed in from `ConversationListContainer`.
- `WorkspaceProvider.tsx` enables the diff stream when
  `!isCreateMode && (!isMobile || mobileTab ∈ {changes, git} || settled || alreadyEnabled)`.
  The flag is sticky per workspace.
- In the mobile branch of `WorkspacesLayout.tsx`, a `visitedTabs` state
  (reset when `workspaceId` changes) gates `PreviewBrowserContainer` and
  `BrowserPanelContainer`.

### F. Measurement and documentation

- `/tmp/probe/probe.js` (Playwright with the Nix Chromium) records socket
  open and first-frame times, history GETs and time until rows. It runs
  without added latency and with 150 ms RTT added through CDP. WebKit is not
  installed, which is recorded in the report.
- `wiki/awaited-stream-settlement.md` (HTTP rules and the handshake-cost
  rule), `wiki/INDEX.md`.

## Data Model
See `./data-model.md`.

## Contracts
See `./contracts/execution-process-log-snapshot.md`.

## Research Notes
See `./research.md`.

## Constitution Check

- **XLIV (new)**: Finite reads use HTTP with a settled flag and a deadline (A,
  B). Sockets are shared per identity with a linger (D). Hidden panels defer
  (E). Each item is loaded once per scope (C). Complies.
- **XL**: The HTTP loader settles once. Close and timeout mean failure. A
  failed turn is skipped, counted and retryable, with no auto-retry during the
  initial load (B, using the existing `loadProcessesInOrder`). Complies.
- **XXXIX**: The snapshot never follows a live tail. A live store is
  snapshotted and reported as `complete: false` (A.2). Complies.
- **XLIII**: The history request carries its own deadline and
  cancellation (B). Complies.
- **II**: Rust unit tests cover the source selection with a pending fallback
  under a timeout. Vitest covers the loader, the cache and the shared stream.
- Constraints: generated types are regenerated, not edited. No new
  dependencies. `pnpm run format` runs at the end.

No deviations.

## Risks & Dependencies

- **Changing `useJsonPatchWsStream` touches every stream consumer.**
  Mitigation: keep the hook's signature and return shape the same, keep the
  existing reconnect test green, and share only when consumers request
  identical keys.
- **Host scope in the key.** A stream lingering under host A must not serve
  host B. The key includes the resolved host.
- **Scope of the per-scope cache.** A process that is reset or deleted
  disappears from the process list. Cached entries are keyed by process id and
  dropped with the scope, so they cannot resurrect it.
- **After-deploy verification** depends on a deploy through the homelab
  module and a real phone load, which is outside this repo.
