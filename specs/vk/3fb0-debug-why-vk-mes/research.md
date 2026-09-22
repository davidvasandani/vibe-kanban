# Research Notes: vk/3fb0-debug-why-vk-mes

## How the hang was confirmed

Read, not inferred, in this order:

1. `crates/server/src/routes/execution_processes.rs:148` —
   `build_recent_messages_response` awaits `normalized_entries` with no timeout
   and no liveness branch.
2. `crates/services/src/services/container.rs:1827` — `normalized_entries`
   drains until `LogMsg::Finished`.
3. `container.rs:1475` — for a live store the stream is
   `history_plus_stream().filter(JsonPatch | Err).chain(once(Finished))`.
4. `crates/utils/src/msg_store.rs:111` — `history_plus_stream` is
   `hist.chain(live)` where `live` is a `BroadcastStream` over
   `sender.subscribe()`. It ends only when the sender drops.
5. `msg_store.rs:92` — `push_finished()` pushes `LogMsg::Finished`, which step
   3's `.filter()` then discards. Termination therefore does **not** come from
   the sentinel; it comes from the sender being dropped when the container
   service removes the store at turn end.

Conclusion: the read returns exactly when the turn ends. That is the timeout.

## Why only running executions are affected

With no live store, `stream_normalized_logs` takes either
`replay_materialized_log` (a `stream::iter` over stored patches, chained with
`Finished` — finite) or historical re-normalization (bounded, chained with
`Finished` — finite). Both terminate, so finished reads have always worked.
This matches the reported symptom exactly and explains why the endpoint looked
healthy in any test that read a completed turn.

## Decisions

### D-1: Snapshot `get_history()` rather than time-bounding the drain

`MsgStore::get_history()` (`msg_store.rs:100`) is a synchronous, lock-guarded
clone of retained history — terminating by construction, no new concept.

Rejected: `tokio::time::timeout` around the existing drain. It converts a hang
into a slow, non-deterministic partial read; every running-execution call pays
the full timeout; and the result depends on scheduling rather than on state.
Constitution XXXVIII names this explicitly as not an acceptable shortcut.

### D-2: Do not stop filtering `Finished` in `stream_normalized_logs`

Letting the real `Finished` through would change what the websocket subscriber
receives (spec FR-5, KB: "The active-process normalized-log WebSocket remains
independent and continues streaming"). It also would not fix anything: a
running turn has not pushed `Finished` yet, so the read would still wait.

### D-3: Free functions, not methods

`cache_execution_from_history` is already split out of
`ContainerService::cache_finished_execution` with the documented reason "so a
finished turn's write-on-completion path can be exercised without a database".
Every existing test in this module's `#[cfg(test)]` block follows that shape —
they exercise `replay_materialized_log` and `cache_execution_from_history`
directly and never construct a `ContainerService`. Following the same pattern
makes the running-execution regression testable without a DB or a live
deployment.

### D-4: No new response field

See `clarifications.md` C-1. Adding a truncation flag would widen a generated
TS type for a 100 MB edge case the fix does not introduce.

## Knowledge base alignment

- `lazy-loading-normalized-conversation-history.md` — "MCP settled-projection
  reads": the MCP tools are one-shot projections over `normalized_entries` and
  must preserve materialization, identity, role filtering and truncation. The
  page specifies only completed executions; the running case it never covers is
  this bug. Constraint honoured: reuse the normalizer, add no second read path.
- `authoritative-snapshot-stream-handoffs.md` — snapshot readers and live
  subscribers are separate contracts, and final output is a reconciliation
  trigger rather than exit evidence (supports FR-9 / clarification C-2).

## Dependencies

None added.
