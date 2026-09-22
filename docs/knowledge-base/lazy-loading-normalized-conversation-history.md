# Lazy-loading normalized conversation history

Tags: `65ab-lazy-load-vk-wor`, `vk/6df4-loading-chat-pin`, `vk/29d8-vk-list-all-mess`,
`vk/3fb0-debug-why-vk-mes`

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

## "Settled" excludes running turns — the source must terminate

The settled-projection reads above were specified for *completed* executions.
Applying the same drain to a **running** one hung the `/messages` endpoint and
both MCP message tools until the turn ended (`vk/3fb0-debug-why-vk-mes`).

`normalized_entries` consumed `stream_normalized_logs` until `LogMsg::Finished`.
For a live `MsgStore` that stream is `history_plus_stream()`: retained history
chained onto a broadcast subscription. Two properties combined into the hang.

- The live half ends only when the broadcast **sender drops**, which happens
  when the turn finishes and the store leaves the container service's map.
- The `Finished` the store pushes at turn end is **discarded by the
  `JsonPatch`-only filter** in `stream_normalized_logs`. A sentinel a
  downstream stage can filter out is not a termination guarantee; only the
  synthetic chained one is reachable, and only after the live half ends.

The rule: a request-scoped read shares the live subscriber's *normalization*,
never its *termination condition*. Snapshot the buffered history
(`MsgStore::select_history`) for a live store; keep draining only the sources
that are finite by construction — sidecar replay and bounded historical
re-normalization. Returning a running turn's partial conversation is correct,
and the response's own `status` is what distinguishes partial from settled.
Constitution XXXVIII.

### Testing a non-terminating read

A test that calls the snapshot helper directly proves nothing: it stays green
if the live-store preference is deleted, because the helper is not where the
choice lives. Make the *source selection* a function that takes the live store
and a lazily-opened fallback stream, then assert with a never-yielding fallback
(`futures::stream::pending()`) that the fallback is never opened, under
`tokio::time::timeout`. A reintroduced wait then fails as a deadline, and the
sabotage check — delete the branch, watch it fail — is what proves the guard.
Wrapping a *synchronous* call in `std::future::ready` inside a timeout is a
false guard: the deadline can never fire.

### Two costs a per-poll snapshot must avoid

- **Whole-history cloning.** `get_history()` deep-copies retained history —
  mostly raw stdout — while holding the lock `push` needs, charging the log
  forwarder for every orchestrator poll. Select inside the read guard
  (`select_history`) so only the wanted variant is cloned.
- **All-or-nothing materialization.** Retained history is byte-capped
  (100 MB) and evicts from the front, so the retained patch run starts
  mid-conversation. Its indices no longer line up with a fresh
  `{"entries": []}` document and strict materialization yields *no* entries.

  Skipping non-applying patches in place does **not** fix this — it looks like
  it does, and silently returns zero entries. Once `add /entries/0..k` are
  evicted, every surviving `add /entries/N` is itself out of bounds, so the
  lenient pass skips all of them too. The mitigation has to re-base: append
  each surviving `add` and remap later `replace`/`remove` onto its new
  position (`materialize_entries_rebased`). Verify it against a realistic
  patch run — monotonic indices, every `replace` after its own `add` — because
  a test that puts a `replace` before its `add` passes over a mitigation that
  does not work. Assert too that an intact history re-bases to exactly what
  strict application gives, so the fallback cannot alter a normal read. The
  stored-sidecar path keeps the strict form, where a patch that does not apply
  means a corrupt artifact that must be re-derived.

## Design gates before product code

Choose the materialization storage/crash-atomicity boundary (SQLite versus an
atomic indexed sidecar), and fully specify revision assignment, resume retention,
and lag recovery before implementing their respective layers.
