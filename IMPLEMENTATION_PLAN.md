# Implementation plan: stop `/messages` hanging on running executions

## Summary

`ContainerService::normalized_entries` drains a live log tail that only ends
when the turn ends. Give it a terminating source for the live-store case by
reusing the buffered-history snapshot the repo already uses in
`cache_execution_from_history`.

## Step 1 — Extract the buffered-patch helper

`crates/services/src/services/container.rs`

`cache_execution_from_history` (line ~138) already does exactly the filter we
need:

```rust
msg_store.get_history().into_iter().filter_map(|msg| match msg {
    LogMsg::JsonPatch(patch) if is_indexed_entry_patch(&patch) => Some(patch),
    _ => None,
})
```

Lift that into a free function `indexed_entry_patches_from_history(&MsgStore)
-> Vec<Patch>` and have `cache_execution_from_history` call it. Pure
refactor, no behaviour change.

## Step 2 — Make `normalized_entries` snapshot the live store

`crates/services/src/services/container.rs` (~line 1827)

Before falling back to the stream drain, check for a live in-memory store:

```rust
async fn normalized_entries(&self, id: &Uuid) -> Option<Vec<NormalizedEntry>> {
    let patches = if let Some(store) = self.get_msg_store_by_id(id).await {
        // A running execution's stream is a live tail: it only terminates
        // when the turn does. Snapshot the buffered history instead.
        indexed_entry_patches_from_history(&store)
    } else {
        // No live store: stream_normalized_logs is finite by construction
        // (cache replay or bounded historical re-normalization).
        ...existing drain loop...
    };
    ...existing materialize_entries(&patches) tail...
}
```

The `materialize_entries` + `normalized_entry_from_patch_value` tail is shared
by both branches and is not duplicated.

Note the ordering matches `stream_normalized_logs`, which also checks
`get_msg_store_by_id` first — so the two paths agree on which source is
authoritative for a given execution.

## Step 3 — Regression tests

`crates/services/src/services/container.rs` `#[cfg(test)]`

1. `normalized_entries_returns_buffered_patches_while_running` — register a
   `MsgStore`, push a couple of `/entries/<n>` patches, do **not** push
   `Finished`, do not drop the store. Assert `normalized_entries` returns
   those entries. Wrap in `tokio::time::timeout` so a regression fails as a
   timeout rather than hanging the suite.
2. Assert repo-diff patches (`/entries/<repo>/<file>`) are still excluded, and
   that a store with no patches yields an empty vec rather than `None`.
3. Keep/confirm the existing finished-execution coverage still passes
   (cache replay path untouched).

Also add a route-level test in
`crates/server/src/routes/execution_processes.rs` only if a `DeploymentImpl`
can be built in-test without a live DB; otherwise the service-level tests are
the regression boundary and `project_messages`' existing unit tests already
cover projection.

## Step 4 — Verify

- `cargo test --workspace`
- `pnpm run check`
- `pnpm run lint`
- `pnpm run format`

No TS types change (`RecentMessagesResponse` shape is untouched), so
`generate-types` is a no-op — confirm with `generate-types:check`.

## Risks / rejected alternatives

- **Rejected: wrap the drain in a timeout.** Turns a hang into a slow partial
  read, makes the response non-deterministic, and still burns the timeout on
  every running-execution call.
- **Rejected: stop filtering out `LogMsg::Finished` in
  `stream_normalized_logs`.** That changes the websocket contract (R3) and
  still would not terminate for a genuinely running turn.
- **Lag:** `get_history` is a lock-guarded snapshot of retained history, so it
  cannot observe broadcast lag. History is byte-capped, so a very long running
  turn can have evicted its oldest entries — the same cap the websocket
  consumer already lives with. Acceptable: the alternative today is no answer
  at all.
