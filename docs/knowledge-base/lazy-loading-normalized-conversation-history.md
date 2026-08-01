# Lazy-loading normalized conversation history

Tags: `65ab-lazy-load-vk-wor`

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

This slice pages by completed execution process, not within a process. Finished
process normalization is bounded to the newest 2,000 normalizable messages, so
an individual request no longer grows without limit, but durable materialized
normalized state and the session-scoped cursor contract described above are
still required for lossless history beyond that per-process boundary.

## Design gates before product code

Choose the materialization storage/crash-atomicity boundary (SQLite versus an
atomic indexed sidecar), and fully specify revision assignment, resume retention,
and lag recovery before implementing their respective layers.
