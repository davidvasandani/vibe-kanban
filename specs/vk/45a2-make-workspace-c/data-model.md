# Data model

No persistent schema changes. The normalized-log sidecar format
(`normalized_log_cache`, `CACHE_VERSION = 1`) is reused unchanged.

## ExecutionProcessLogSnapshot (wire, generated TS)

| Field | Type | Meaning |
| --- | --- | --- |
| `entries` | `Array<PatchType>` | The `entries` array the matching `…/ws` replay converges to, in index order. |
| `complete` | `boolean` | True only when the entries are settled: there is no live store, the process is not running, and the settled source ended with `Finished`. |

## LogSnapshot (Rust, services)

Holds `entries: Vec<serde_json::Value>` and `complete: bool`. It is the
`ContainerService` return type that the route maps onto the wire type.

## Client state

- **Settled-entries cache** (per conversation scope): `Map<processId, PatchType[]>`
  for complete snapshots, plus a `Map<processId, Promise>` for requests in
  flight. It is discarded when the scope changes.
- **Shared stream registry** (module-level): `Map<key, SharedJsonPatchStream>`,
  where key = endpoint + `|` + resolved host scope. Each stream holds its
  subscribers, `{data, isConnected, isInitialized, error}`, the socket, the
  retry state, the `finished` flag and the linger timer.
- **Chat-history-ready store**: `Set<workspaceId>` of workspaces whose chat
  has emitted its first settled `'initial'` view in this page session.
