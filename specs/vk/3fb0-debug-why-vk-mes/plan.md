# Implementation Plan: Message reads return while a turn is still running

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context

- Rust workspace; the defect is in `crates/services` and surfaces through
  `crates/server`.
- Axum handler: `crates/server/src/routes/execution_processes.rs`
  — `get_execution_process_messages` → `build_recent_messages_response`.
- Service: `crates/services/src/services/container.rs` —
  `ContainerService::normalized_entries` (~line 1827) and
  `ContainerService::stream_normalized_logs` (~line 1470).
- Log buffer: `crates/utils/src/msg_store.rs` — `MsgStore`, `get_history`
  (line 100), `history_plus_stream` (line 111), 100 MB retained-history cap
  (line 13).
- No frontend, schema, or generated-type change. `RecentMessagesResponse` is
  untouched, so `pnpm run generate-types` is a no-op.

## Root cause

`normalized_entries` loops over `stream_normalized_logs` until it sees
`LogMsg::Finished`. For an execution with a live in-memory `MsgStore`, that
stream is:

```rust
store.history_plus_stream()
    .filter(|msg| future::ready(matches!(msg, Ok(LogMsg::JsonPatch(..)) | Err(_))))
    .chain(futures::stream::once(async { Ok(LogMsg::Finished) }))
```

`history_plus_stream` is buffered history chained to a live `BroadcastStream`.
The `.filter()` discards every non-`JsonPatch` message — including the genuine
`LogMsg::Finished` the store pushes at turn end. The only reachable `Finished`
is the synthetic chained one, which requires the live half to terminate, which
requires the broadcast sender to be dropped, which happens when the container
service drops the store at turn end. So the read blocks for the remaining
lifetime of the turn. Finished executions take the cache-replay or historical
branches, both finite — which is why only running reads hang.

## Architecture & Approach

Give the one-shot reader a self-terminating source; leave the subscriber alone.

### Step 1 — Shared patch filter (pure refactor)

`cache_execution_from_history` (container.rs ~138) already contains the exact
filter needed. Lift it into a free function:

```rust
fn indexed_entry_patches_from_history(msg_store: &MsgStore) -> Vec<Patch>
```

and call it from `cache_execution_from_history`. No behaviour change.

### Step 2 — Shared materialization tail

Both branches of `normalized_entries` end identically (materialize patches,
deserialize entries, warn on failure). Lift that into:

```rust
fn entries_from_patches(id: &Uuid, patches: &[Patch]) -> Option<Vec<NormalizedEntry>>
```

so the new branch reuses it rather than duplicating it (constitution III/VI).

### Step 3 — DB-free live-store snapshot

```rust
fn normalized_entries_from_history(id: &Uuid, msg_store: &MsgStore)
    -> Option<Vec<NormalizedEntry>>
```

= `entries_from_patches(id, &indexed_entry_patches_from_history(msg_store))`.

Split out as a free function deliberately, mirroring the existing precedent in
this file: `cache_execution_from_history` is documented as "Split out so a
finished turn's write-on-completion path can be exercised without a database".
The same reasoning applies here and is what makes the regression test possible
without standing up a `ContainerService`.

### Step 4 — Use it in `normalized_entries`

```rust
async fn normalized_entries(&self, id: &Uuid) -> Option<Vec<NormalizedEntry>> {
    // A live store's stream is a tail: it only ends when the turn does.
    // Snapshot its buffered history instead of following it.
    if let Some(store) = self.get_msg_store_by_id(id).await {
        return normalized_entries_from_history(id, &store);
    }
    // No live store: stream_normalized_logs is finite by construction
    // (cache replay, or bounded historical re-normalization).
    ...existing drain, ending in entries_from_patches...
}
```

Checking `get_msg_store_by_id` first matches `stream_normalized_logs`' own
ordering, so both paths agree on which source is authoritative.

### Requirement mapping

| Req | Where satisfied |
| --- | --- |
| FR-1, FR-2 | Step 4 — snapshot returns immediately |
| FR-3 | Already: handler copies `execution_process.status` / `exit_code` |
| FR-4, FR-7 | Steps 2/4 — finished path and `project_messages` untouched |
| FR-5 | `stream_normalized_logs` not modified |
| FR-6 | Live-store branch does no cache or normalization work |
| FR-8 | `entries_from_patches` returns `None` on patch failure; handler's `unwrap_or_default()` yields an empty list |
| FR-9 | Already: `last_assistant_message` over the returned entries |

## Data Model

See `./data-model.md`. No persisted schema change; the document covers the
in-memory patch → entry materialization the fix relies on.

## Contracts

See `./contracts.md`. The HTTP/MCP response shape is unchanged; the document
records the behavioural matrix that changes (running reads) and what must not.

## Research Notes

See `./research.md`.

## Constitution Check

Against `.specify/memory/constitution.md` v0.34.0:

- **XXXVIII (request-scoped reads terminate independently of liveness)** — the
  principle added for this task, and the direct statement of the fix: share the
  normalization, not the termination condition.
- **I (clarity over cleverness)** — a source swap plus a comment naming the
  hazard, not a timeout or sentinel workaround.
- **II (test the contract)** — regression test asserts the still-running read
  under `tokio::time::timeout`, so a reintroduced wait fails rather than hangs.
- **III / VI (small steps, don't rebuild)** — reuses `get_history` and the
  existing patch filter; adds no new read path, storage, or dependency.
- **XXXI (historical views materialized once)** — untouched: the cache-replay
  and single-flight historical branches keep their behaviour; the new branch
  only intercepts the case where no reconstruction is needed at all.
- **XIX (observability is read-only)** — the read remains side-effect-free; the
  live-store branch writes no sidecar.

No deviations.

## Risks & Dependencies

- **History eviction (100 MB cap).** A turn that evicts an `add` while keeping a
  later `replace` makes `materialize_entries` fail → empty list, promptly. Worse
  than a full answer, strictly better than today's hang. Recorded as FR-8.
- **Store-removal race.** If the store is dropped between the
  `get_msg_store_by_id` check and the snapshot, we hold an `Arc` and still read
  its history, so the read succeeds from a consistent snapshot. The next read
  takes the cache path. No hang either way.
- **Test could pass vacuously** if written against a store that has been
  finished or dropped. The test must keep the store alive and never push
  `Finished` — assert the hazard, not a happy path.
- No new dependencies. `tokio::time::timeout` is already available in tests.
