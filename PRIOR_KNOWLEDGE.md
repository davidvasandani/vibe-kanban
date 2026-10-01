# Prior knowledge: workspace chat load with expensive WebSocket handshakes

Task: `vk/45a2-make-workspace-c`. This file pulls together what the project
knowledge bases (`wiki/` and `docs/knowledge-base/`) already say about this
problem. The knowledge bases were only read in this stage, not changed.

## Relevant pages

- `wiki/awaited-stream-settlement.md` (`vk/5f70-not-loading-chat`)
- `docs/knowledge-base/lazy-loading-normalized-conversation-history.md`
  (`65ab-lazy-load-vk-wor`, `vk/6df4-loading-chat-pin`, `vk/3fb0-debug-why-vk-mes`)
- `docs/knowledge-base/authoritative-snapshot-stream-handoffs.md`
  (`vk/3488-fix-stale-execut`, `vk/113f-sidebar-randomly`)
- `wiki/workspace-carousel-view.md` (several chats at once, so socket count
  per chat matters)
- `wiki/coordinator-nfs-load.md` (cold git diff reads over NFS are slow;
  #350 bounded the summaries path, which must not be touched)

## What we already know

1. **An awaited log fetch settles exactly once.** `finished` means success.
   Close, error, parse failure, open failure and idle timeout all mean
   failure. The idle deadline applies only to settled history, never to a
   running turn. A failed turn is skipped, counted and retryable through "load
   earlier", with no auto-retry during the initial load. One unsettled fetch
   must never hold the spinner. All of these carry over to the HTTP path, with
   an `AbortController` deadline in place of the idle timer.
2. **The server already has a finite settled source.** A finished process is
   served from its materialized normalized-log sidecar. On a miss, it does one
   bounded historical normalization (newest 2,000 messages) under a
   per-execution lease and a global permit of 1, then writes the sidecar.
   `ContainerService::normalized_entries` reads the same sources.
3. **A request-scoped read must never follow a live store's tail**
   (`vk/3fb0`). The tail only ends when the turn does, and the store's own
   `Finished` is filtered out. Snapshot the buffered history with
   `select_history`, and let `status` (here, `complete: false`) distinguish a
   partial read from a settled one. Test the source selection with a
   never-yielding fallback under `tokio::time::timeout`.
4. **Cancellation drops the historical normalizer.** The lease and permit are
   tied to the stream's lifetime, so a dropped reader aborts materialization.
   For HTTP that means an aborted client request would cancel a cold
   normalization unless the drain is detached.
5. **Frontend history invariants** (`65ab`): load on top intersection or an
   explicit action, through one single-flight path. Every result is scoped to
   a generation. Results commit in request order, never completion order.
   Concurrency only tunes latency, never what is shown.
6. **Snapshot streams keep their state during transport loss** and replace it
   with a full snapshot on reconnect. Only `Ready` resets backoff. A sharing
   layer must keep these properties per stream identity (endpoint plus host
   scope).
7. **Debugging recipe**: talk to the coordinator directly at
   `http://172.16.100.102:3334`. Workers have Node 24. Only
   `ConversationList`'s spinner sits inside `.w-chat`.

## Gaps this task fills

- No page yet records that WebSocket *count* is itself the cost behind
  Cloudflare and iOS Safari, or that history should go over HTTP.
- Nothing yet shares identical JSON-patch streams across components.
