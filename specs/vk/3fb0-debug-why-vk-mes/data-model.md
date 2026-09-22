# Data Model: vk/3fb0-debug-why-vk-mes

No persisted schema change. No migration. No generated-type change
(`RecentMessagesResponse` and `SessionMessage` are untouched, so
`pnpm run generate-types` is a no-op).

The relevant model is the in-memory pipeline the read materializes.

## Pipeline

```
MsgStore.history : VecDeque<StoredMsg>      (capped at 100 MB, front-evicting)
  └─ LogMsg::JsonPatch(Patch)
       └─ filter: is_indexed_entry_patch      /entries/<usize> only
            └─ materialize_entries            apply onto {"entries": []}
                 └─ normalized_entry_from_patch_value
                      └─ NormalizedEntry { timestamp, entry_type, content, metadata }
```

## Entities

| Entity | Where | Notes |
| --- | --- | --- |
| `LogMsg` | `crates/utils/src/log_msg.rs` | `JsonPatch`, `Finished`, `Stdout`, `SessionId`, … |
| `StoredMsg` | `crates/utils/src/msg_store.rs` | `{ msg, bytes }`; `bytes` drives cap eviction |
| `Patch` | `json_patch` | Only `/entries/<index>` ops are conversation entries |
| `NormalizedEntry` | `crates/executors/src/logs` | The projected message |
| `SessionMessage` | `crates/server/src/routes/execution_processes.rs` | API shape; id is `{execution_id}:{index}` |

## Patch scoping

`is_indexed_entry_patch` (`container.rs:188`) admits only
`/entries/<numeric>`. Repo-diff patches target `/entries/<repo>/<file>`, a
nested object that cannot be applied against the `{"entries": []}` array
document, and are not messages. This filter is unchanged by this work and must
keep applying to the running-execution path — covered by a regression test.

## Ordering and identity

Entry order is patch-application order; identity is
`{execution_id}:{index}` assigned in `project_messages` from the position in
the materialized vector. Both are properties of materialization, not of the
source stream, so snapshotting the same patches yields the same identities a
finished read would produce.

## Failure state

If retained history was evicted such that a surviving `replace` has no
corresponding `add`, strict `materialize_entries` returns `CacheError::Patch`.
For a live store the read then retries with `materialize_entries_lossy`, which
applies what still applies and reports how many patches it skipped, so the
surviving messages are returned rather than none (FR-8). `json_patch::patch`
keeps an undo stack and restores the document on failure, so a skipped patch
leaves earlier entries intact. Only if the document itself is malformed does
the read yield `None`, which the handler's `unwrap_or_default()` turns into an
empty list. Prompt in every case, never blocking.
