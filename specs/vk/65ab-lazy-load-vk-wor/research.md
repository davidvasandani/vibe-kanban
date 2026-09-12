# Research: Lazy-load workspace chat history

## Current transport and storage

- `packages/web-core/src/features/workspace-chat/model/hooks/useConversationHistory.ts`
  loads completed execution processes newest-first, but each process WebSocket
  replays its complete log. After the initial threshold it automatically loads
  every older process in background batches.
- `crates/server/src/routes/execution_processes.rs` exposes only per-process raw
  and normalized WebSocket streams. Neither accepts a cursor or limit.
- `crates/services/src/services/container.rs::stream_normalized_logs` serves an
  in-memory `MsgStore` for a running process. For a completed process it reloads
  persisted raw messages, reruns the executor normalizer, deduplicates adjacent
  patches, and streams the result.
- `crates/services/src/services/execution_process.rs::spawn_stream_raw_logs_to_storage`
  persists stdout/stderr but explicitly skips `JsonPatch`; final normalized
  state is therefore not durably indexed.
- JSON Patch operations can add, replace, and remove `/entries/{index}`. A
  reverse slice of raw frames is not a correct tail because earlier operations
  determine the final state and normalizer lifecycle.
- Commit `a9622cfd` made abandoned historical normalization cancellable and
  concurrency-bounded. A new page path must retain those properties.

## Decision: persist materialized normalized entries

Persist the final process-local normalized entry state (including stable
absolute index and a monotonically increasing revision/watermark) as the live
normalizer emits patches. Page reads can then use a bounded reverse range
without rerunning vendor normalizers or replaying raw history.

For pre-feature completed processes, a service-owned rollout worker reuses the
existing cancellable/concurrency-bounded normalizer, writes the cache atomically,
and marks the transcript page-ready. It may be triggered when sessions are
listed or by an application startup queue, but it is not performed inside an
interactive page request. Until ready, the page API returns an observable
preparing/retry response. No homelab change is needed.

Alternatives rejected:

- **Frontend slicing only:** still performs full backend replay/transfer and
  retains full source state.
- **Reverse-read raw JSONL:** normalizers are stateful and later replacements
  depend on earlier adds/tool lifecycle.
- **Paginate patch frames:** a page beginning with `replace /entries/7` is not
  independently materializable and frame count does not equal visible history.
- **Process-only pagination:** one process can contain thousands of entries, so
  the initial bound would still fail.
- **Build legacy state inside the first page request:** makes the response look
  bounded while retaining the exact backend replay cost this feature is meant
  to remove.

## Decision: session-level HTTP history pages

Add a session-scoped bounded request/response API for historical pages. Keep the
existing per-process WebSocket as the live channel, extended with a snapshot
watermark so the client can ignore live revisions already represented in its
tail snapshot.

HTTP fits finite retryable pages and structured errors; WebSocket remains useful
for unbounded live continuation. The existing signed/remote routing layer must
protect both.

## Decision: cursor contents and validation

The client sees an opaque base64url cursor. Its decoded versioned payload binds:

- session id;
- ordered boundary `(process_created_at, process_id, entry_index)`;
- snapshot generation/revision;
- schema version.

The server validates the route session and current authorization context against
the cursor. Process UUID breaks timestamp ties. Cursor format is not a client
contract and can evolve by version.

## Decision: live handoff

The initial page includes a per-running-process revision watermark. The live
subscription is established before the page result is committed; queued live
patches at or below the watermark are discarded, and higher revisions are
applied in order. If the transport cannot supply/replay the required revision,
the client re-requests the bounded tail rather than silently accepting a gap.

## Decision: scroll anchoring

Reuse `ConversationListContainer`'s semantic row keys and measurement-aware
scroll helpers. Capture the first visible row key and its offset before calling
`loadEarlier`; after prepend/render, find the same key and adjust by the measured
offset delta. Do not anchor by array index because prepends and aggregation
change indices.

## Dependencies

No new external dependency is needed. Existing serde/base64 support should be
reused if present; otherwise an opaque cursor can be encoded with an existing
URL-safe codec in the workspace rather than adding a top-level package.
