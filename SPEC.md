# Spec: `/messages` API hangs on running executions

## Problem

`GET /api/execution-processes/{id}/messages` (and the MCP tools
`list_recent_messages` / `list_all_messages` that wrap it) never returns while
the target execution is still `Running`. The caller sees a client-side timeout.

## Root cause

`build_recent_messages_response` (`crates/server/src/routes/execution_processes.rs`)
calls `ContainerService::normalized_entries`, which drains
`stream_normalized_logs` until it observes `LogMsg::Finished`:

`crates/services/src/services/container.rs:1827`

```rust
let mut stream = self.stream_normalized_logs(id).await?;
while let Some(item) = stream.next().await {
    match item {
        Ok(LogMsg::JsonPatch(patch)) => { ... }
        Ok(LogMsg::Finished) => break,
        ...
    }
}
```

For an execution with a live in-memory `MsgStore`, `stream_normalized_logs`
(`container.rs:1475`) returns:

```rust
store.history_plus_stream()
    .filter(|msg| future::ready(matches!(msg, Ok(LogMsg::JsonPatch(..)) | Err(_))))
    .chain(futures::stream::once(async { Ok(LogMsg::Finished) }))
```

Two properties combine into the hang:

1. `history_plus_stream` (`crates/utils/src/msg_store.rs:111`) is buffered
   history **chained to a live `BroadcastStream`**. That live half only ends
   when the broadcast sender is dropped — i.e. when the execution finishes and
   the container service drops the store from its map.
2. The `.filter()` discards everything that is not a `JsonPatch`, so the real
   `LogMsg::Finished` the store pushes at process end is **removed**. The only
   `Finished` the consumer can ever see is the synthetic chained one, which is
   reached solely by the live stream terminating.

So `normalized_entries` blocks for the entire remaining duration of the turn.
This is correct for the websocket consumer (it *wants* a live tail) but wrong
for the `Vec`-returning snapshot path.

The finished-execution paths (materialized cache replay, historical
re-normalization) do terminate, which is why the endpoint only hangs for
running turns — the exact case the MCP tool documents as its purpose
("Check this before a follow-up `run_session_prompt`").

## Requirements

- R1: `/messages` must return promptly for an execution in any state,
  including `Running`.
- R2: For a running execution it must return the messages normalized **so
  far**, not an error and not an empty list.
- R3: The websocket log stream must keep its live-tail behaviour unchanged.
- R4: Finished-execution behaviour (cache replay, historical normalization,
  `has_more`, role filtering, truncation) must be unchanged.

## Approach

Give `normalized_entries` a terminating source instead of the live tail. When
an in-memory `MsgStore` exists, read the already-buffered patches via the
existing synchronous snapshot `MsgStore::get_history()`
(`crates/utils/src/msg_store.rs:100`) and materialize from those. Fall back to
the existing `stream_normalized_logs` drain only when there is no live store,
where the stream is finite by construction.

This keeps one normalization pipeline, touches no websocket behaviour, and
needs no timeout heuristic.

## Out of scope

- Changing the websocket streaming contract.
- Changing the normalized-log cache/materialization design.
- Pagination or limit semantics.

## Acceptance

- A running execution with buffered patches returns those messages immediately.
- Regression test that fails (hangs) before the change and passes after.
- `cargo test --workspace`, `pnpm run check`, `pnpm run lint` pass.
