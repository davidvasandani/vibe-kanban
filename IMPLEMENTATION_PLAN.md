# Implementation plan: workspace chat loads fast over expensive handshakes

Task `vk/45a2-make-workspace-c`. See `SPEC.md` for the diagnosis and the
design, and `PRIOR_KNOWLEDGE.md` for the rules inherited from earlier tasks.

## Step 1: server log snapshot (`crates/services`)

File: `crates/services/src/services/container.rs`

1. Add `pub struct LogSnapshot { entries: Vec<serde_json::Value>, complete: bool }`.
2. Extract the source choice into a free function
   `normalized_log_snapshot_from_sources(id, live_store, status_running, settled_stream)`:
   - With a live store, materialize `indexed_entry_patches_from_history`
     (falling back to rebased materialization) and set `complete: false`.
   - Otherwise drain the finite settled stream, keep only indexed patches,
     `materialize_entries` them, and set `complete = !status_running`. Return
     `None` when the stream is `None`. A stream error or a missing `Finished`
     returns `None`, never a partial complete snapshot.
3. Add a trait method `normalized_log_snapshot(&self, id, status_running)`.
4. Add `raw_log_snapshot_from_sources` / `raw_log_snapshot`, which turns
   stdout and stderr into `{type: 'STDOUT'|'STDERR', content}` in order. With a
   live store it snapshots history and returns `complete: false`. Otherwise it
   loads the stored raw messages.
5. Unit tests: a live store never opens the fallback (pending stream under
   timeout); the settled stream materializes; a stream without `Finished` is
   not complete; a running status is not complete; raw ordering.

## Step 2: routes (`crates/server`)

File: `crates/server/src/routes/execution_processes.rs`

1. `ExecutionProcessLogSnapshot { entries: Vec<JsonValue> (TS: Array<PatchType>), complete: bool }`
   with `#[derive(Serialize, TS)]`.
2. `GET /{id}/normalized-logs` and `GET /{id}/raw-logs`. Each spawns the
   snapshot in a `tokio::spawn`, so a client abort does not cancel a cold
   materialization, and awaits the join handle. A `None` result returns
   `complete: true` with no entries for processes with nothing to show. This
   matches the socket, which sends `finished` immediately when there are no
   logs.
3. Register the type in `crates/server/src/bin/generate_types.rs` and run
   `pnpm run generate-types`.

## Step 3: HTTP history loader (web-core)

New file: `packages/web-core/src/shared/hooks/useConversationHistory/fetchHistoricEntries.ts`

- `fetchProcessLogSnapshot(process, { deadlineMs, signal, request })`
  wraps `makeLocalApiRequest` with an `AbortController` deadline and settles
  once. It rejects on timeout, abort, non-2xx, `success: false` and bad JSON.
- `loadHistoricProcessEntries(process, deps)` uses HTTP first. On
  `complete: false` it falls back to the socket loader. It returns
  `{ entries, complete }`.
- Constant `HISTORY_HTTP_DEADLINE_MS = 30_000` in `constants.ts`.
- Tests: `fetchHistoricEntries.test.ts`.

## Step 4: history once per scope (web-core)

`useConversationHistory.ts`:

- `settledEntriesCacheRef: Map<processId, PatchType[]>`, cleared on scope
  change.
- `loadEntriesForHistoricExecutionProcess` → cache hit, or the HTTP loader
  (store the result when it is complete).
- The running → finished reload bypasses the cache and replaces the entry.
- A pure helper `createHistoricEntriesLoader(fetcher)` in
  `conversation-history-paging.ts` holds the cache and in-flight dedup, so
  "loaded once per scope" can be tested without React.

## Step 5: shared JSON-patch streams (web-core)

New file: `packages/web-core/src/shared/lib/sharedJsonPatchStream.ts`

- Registry keyed by `endpoint + resolved host scope`. Subscribe/unsubscribe
  with ref counting. The socket opens on the first subscribe and closes
  `STREAM_LINGER_MS` after the last unsubscribe.
- Reconnect, backoff, `Ready`, `finished` and the clean-close rules move over
  from `useJsonPatchWsStream`.
- `useJsonPatchWsStream` becomes a `useSyncExternalStore` adapter. The
  `injectInitialEntry` and `deduplicatePatches` options (unused in the repo)
  are kept in the key via a per-hook fallback: when present, the stream is
  private (unshared).
- `useExecutionProcesses` always sets `show_soft_deleted=true` and filters
  `dropped` on the client unless `showSoftDeleted`.
- `agentsApi.getDiscoveredOptionsStreamUrl` drops `workspace_id` and
  `repo_id` when `session_id` is present.
- Tests: the shared stream (one socket for N consumers, linger, reconnect
  keeps the snapshot) and `useExecutionProcesses` (one socket for plain and
  soft-deleted consumers, with filtering).

## Step 6: mobile deferral (web-core)

- `WorkspacesLayout` mobile branch: track visited tabs. Mount
  `PreviewBrowserContainer` / `BrowserPanelContainer` only once visited.
- `useMobileDiffStreamGate`: a small zustand store (`useChatLoadGateStore`)
  with `chatHistorySettled(workspaceId)`, set by `useConversationHistory` on
  its first non-loading `'initial'` emit. `WorkspaceProvider` enables the diff
  stream when `!isMobile || tab ∈ {changes, git} || settled`, and stays
  enabled once it has been enabled.

## Step 7: measure, document, verify

- Re-run `/tmp/probe/probe.js` against a local build of the change: the
  server binary on a spare port serving the built frontend, with the same
  database? Not possible without a copy of production data. Instead, deploy
  after merge and re-run the probe against the coordinator. Before merge,
  verify with unit tests and a local dev instance probe.
- Update `wiki/awaited-stream-settlement.md` (HTTP rules and the WebSocket
  cost rule) and `wiki/INDEX.md`.
- `pnpm run check`, `pnpm run lint`, `cargo test --workspace`,
  `pnpm run format`.
