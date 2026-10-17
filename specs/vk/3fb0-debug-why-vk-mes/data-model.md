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

Front eviction makes the retained patch run start mid-conversation, so its
indices no longer line up with a fresh `{"entries": []}` document: strict
`materialize_entries` fails on the very first surviving `add /entries/N`
(N > 0) as out of bounds.

For a live store the read then re-bases with `materialize_entries_rebased`,
which walks the operations directly instead of applying them positionally:

| Operation | Behaviour |
| --- | --- |
| `add /entries/N` | append to the array; record `N -> position` |
| `replace /entries/N` | write at the recorded position, else count as dropped |
| `remove /entries/N` | remove at the recorded position and shift later mappings down, else count as dropped |
| anything else | count as dropped |

An intact history re-bases to exactly what strict application produces, so the
fallback cannot change a normal read (asserted by test). Dropped-operation
count is logged. The read is prompt in every case and never blocking (FR-8).
