# Lazy-loading normalized conversation history

Tags: `65ab-lazy-load-vk-wor`, `vk/6df4-loading-chat-pin`, `vk/29d8-vk-list-all-mess`

## Why frontend virtualization is insufficient

`ConversationListContainer` virtualizes mounted rows, but
`useConversationHistory` still opens a per-process normalized-log WebSocket.
Each completed-process stream reconstructs that process from its beginning, and
the hook's background batching eventually loads every older process. This limits
DOM work only; backend normalization, transfer, derivation, and retained source
state still grow with the transcript.

The current initial threshold is also not a true bound: loading stops only after
a whole process crosses it, so one large execution can exceed it arbitrarily.

## The durable-state prerequisite

Completed execution storage persists raw stdout/stderr and skips normalized
`JsonPatch` messages. `ContainerService::stream_normalized_logs` must therefore
reload the entire raw log and rerun the vendor normalizer before it knows final
entry state.

A correct conversation tail cannot be produced by reverse-reading raw JSONL or
slicing patch frames:

- normalizers carry tool/lifecycle state from earlier events;
- `replace /entries/{index}` depends on the earlier add;
- remove/reset operations can invalidate earlier indexes;
- patch-frame count is not visible-entry count.

Genuine bounded paging requires a durable materialized normalized view keyed by
`(execution_process_id, entry_index)`. Apply add/replace/remove operations before
paging and retain a monotonic revision for live reconciliation.

## Page and live-stream contract

Expose finite history pages at conversation/session scope even if the opaque
cursor contains process-local position. Page final normalized entries—not raw
patches—in chronological order, with:

- a server-enforced maximum;
- opaque, route-scoped continuation state;
- explicit `has_more` rather than length inference;
- deterministic ties using process UUID plus process time/index;
- stable frontend identity `{process_id}:{entry_index}`.

Keep live continuation incremental. The snapshot returns a revision watermark;
the live channel resumes strictly after it or explicitly requires a bounded
resnapshot. Assign/durably apply revision ownership before claiming the event in
a snapshot, and specify broadcast-lag behavior before implementation.

## Legacy rollout and cancellation

Existing transcripts require one full normalization to build the materialized
view. Do that in an observable, capacity-bounded, cancellable rollout path—not
inside an interactive page request that merely returns a small response. Reuse
the existing historical normalization semaphore and abort-on-stream-drop
discipline. Raw logs remain the rebuildable source of truth.

## Single-flight materialization for concurrent readers

Task `vk/6df4-loading-chat-pin` established the coordination order for legacy
executions that do not yet have a valid normalized sidecar:

1. Optimistically replay a valid sidecar without taking coordination or global
   capacity.
2. On a miss, acquire a weakly retained, execution-ID-keyed ownership lease.
3. Recheck the sidecar after ownership transfers, because the prior owner may
   have completed while the reader waited.
4. Only the remaining cache miss competes for the global historical
   normalization permit and reconstructs the vendor log.

The keyed lease must live for the returned stream's lifetime, not merely until
the stream is constructed. This makes concurrent readers of one execution join
one materialization attempt. A successful leader atomically publishes the
sidecar, so waiters replay durable output instead of normalizing again. If the
leader stream is dropped or its task is aborted, cancellation releases both the
global permit and keyed lease; the next waiter rechecks the sidecar and becomes
the retrying leader when necessary.

Keep keyed registry cells weak and remove dead generations with identity-aware
cleanup so a large history does not turn coordination metadata into permanent
memory growth. Global capacity still protects CPU across different executions,
while the keyed lease prevents duplicate CPU for the same execution. Idle host
memory or CPU is not a reason to fan out duplicate reconstruction work; use
available capacity for independent execution IDs and serve completed work from
the sidecar.

## Frontend invariants

- Do not start the next history request just because the recent page finished;
  load on top intersection or an accessible load/retry action.
- Coalesce both triggers through one single-flight function.
- Scope every page and live result to a generation so a session switch cannot
  append stale rows.
- Preserve a semantic row key and viewport offset before prepend, then correct
  after rendering/measurement. Array indexes are unstable across prepend and
  aggregation.
- A failed older page leaves the loaded recent tail usable and retryable.

## Shipped bounded-preload slice

Task `65ab-lazy-load-vk-wor` removed the frontend's idle loop that automatically
opened every older completed-process log stream. Workspace chat now loads its
newest completed processes for the initial view, then requests older processes
only when the top sentinel intersects or the user activates the load/retry
control. Both triggers share a single-flight request; stale session generations
are ignored; failures are isolated per process so older turns remain reachable
and failed turns remain retryable.

Prepending saves the first visible row's semantic key, top offset, and scroll
height. Scroll-height compensation first keeps a virtualized anchor in the
render window, after which semantic-key correction restores its exact offset.
The active-process normalized-log WebSocket remains independent and continues
streaming while historical pages load.

Explicitly loaded older batches are retained while the reader remains away
from the live tail. Once they return to the bottom, the frontend releases those
batches and reconstructs the recent-tail window: all running processes plus the
newest completed processes needed to cross the initial entry threshold, capped
at 20 completed processes so empty/script-only records cannot make retention
unbounded. Released processes remain discoverable and can be loaded again.

This slice pages by completed execution process, not within a process. Finished
process normalization is bounded to the newest 2,000 normalizable messages, so
an individual request no longer grows without limit, but durable materialized
normalized state and the session-scoped cursor contract described above are
still required for lossless history beyond that per-process boundary.

## MCP settled-projection reads

The MCP message tools are one-shot projections over
`ContainerService::normalized_entries`, not independent log readers.
`list_recent_messages` requests a clamped tail (default 20, maximum 100), while
`list_all_messages` explicitly selects every entry in the available settled
projection. Keep this distinction typed at the shared response builder rather
than encoding “all” as a magic limit or weakening the recent-reader cap.

“All” does not mean bypassing historical reconstruction safeguards. A fresh
completed execution can serve its full atomically cached normalized history; a
legacy cache miss still applies the newest-2,000-normalizable-raw-message bound
and emits an omission notice. Both MCP reads preserve normalized patch
materialization, chronological identity, role filtering, per-entry truncation,
single-flight cache-miss coordination, and owning-workspace authorization.

## Design gates before product code

Choose the materialization storage/crash-atomicity boundary (SQLite versus an
atomic indexed sidecar), and fully specify revision assignment, resume retention,
and lag recovery before implementing their respective layers.
