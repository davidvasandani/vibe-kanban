# Contract: Coordinator metrics API

The browser-facing surface. Two routes in
`crates/server/src/routes/cluster_metrics.rs`, mounted from the existing
`admin_router()` (`crates/server/src/routes/workers.rs:33-37`) and therefore
nested under `/api` inside `relay_signed_routes`
(`crates/server/src/routes/mod.rs:63-64, :99`).

Both inherit the existing relay request-signature middleware, response signing,
and origin validation. No new authentication scheme is introduced.

---

## `GET /api/cluster/metrics`

The snapshot / fallback path.

### Request

No parameters. The frontend reaches it through `makeHostAwareRequest`
(`packages/web-core/src/shared/lib/api.ts:197`) so the request targets the
**selected machine**, not whichever host happens to serve the UI.

### Response `200`

`ApiResponse<ClusterMetricsSnapshot>`:

```json
{
  "success": true,
  "data": {
    "generated_at": "2026-08-01T04:32:18.114Z",
    "sample_interval_ms": 2000,
    "nodes": {
      "9abaa372-2f10-4e02-9651-b869455cdc67": {
        "node_id": "9abaa372-2f10-4e02-9651-b869455cdc67",
        "hostname": "think2",
        "role": "coordinator",
        "health": null,
        "availability": { "status": "available" },
        "last_contact_at": "2026-08-01T04:32:18.114Z",
        "latest": { "...": "HostSample, processes populated" },
        "history": [{ "...": "HostSample, processes null" }]
      },
      "1f0c…": {
        "node_id": "1f0c…",
        "hostname": "think4",
        "role": "worker",
        "health": {
          "status": "offline",
          "mount_status": "healthy",
          "lease_expires_at": "2026-08-01T04:31:32.008Z",
          "schedulable": false
        },
        "availability": {
          "status": "unreachable",
          "reason": "connection refused"
        },
        "last_contact_at": "2026-08-01T04:31:02.008Z",
        "latest": { "...": "last known, client renders it as stale" },
        "history": []
      }
    }
  }
}
```

Contract details:

- **`nodes` is an object keyed by `node_id`, not an array.** This is what makes
  the patch stream safe: a worker registering or deregistering mid-stream shifts
  no indices. See `../data-model.md` invariant 3.
- `sample_interval_ms` is served rather than hardcoded on the client, so the
  sparkline x-axis stays correct if the cadence ever changes.
- `role` distinguishes the coordinator, which is synthesised here and has **no**
  `worker_nodes` row.
- `health` is the cluster's own judgement of the node, **separate from**
  `availability` (whether metrics could be read). It is `null` for the
  coordinator, which has no worker row. This is what makes FR-24 satisfiable:
  without it the panel has nothing to agree with Settings *about*.
- **Health is derived read-only.** A row whose `lease_expires_at <= now` is
  reported as `offline` regardless of its stored `status`. This endpoint does
  **not** call `WorkerRegistry::expire_heartbeats` — that issues
  `UPDATE worker_nodes SET status = 'offline'`
  (`crates/db/src/models/worker_node.rs:160-173`), and a monitoring read must
  not trigger a lifecycle transition (FR-21, Constitution XIX, analysis E1).
- Reading this endpoint triggers **one bounded collection round** if the
  collector is idle, then serves the result. Without that, on a cluster whose
  drawer has been shut for more than the retention window this endpoint would
  return `latest: null` for every worker — a "fallback" that falls back to
  nothing. The round is bounded by the same 5s per-node timeout and does not
  start the continuous collector.
- Workers that have never been polled report
  `availability: { "status": "not_collected" }` — distinct from a failure.
- A node whose newest retained sample is older than the retention window has
  `latest: null` and `history: []`, leaving only `availability` and
  `last_contact_at` (FR-18 / C5).

---

## `GET /api/cluster/metrics/ws`

The live path. Snapshot-then-JSON-Patch over a signed WebSocket, using
`SignedWsUpgrade` (`crates/server/src/middleware/signed_ws.rs:33`) and the
`LogMsg::JsonPatch` idiom from `/api/approvals/stream/ws`
(`stream_approvals_ws`, `crates/server/src/routes/approvals.rs:51-106`). Note
that route drives `deployment.approvals().patch_stream()` straight onto the
socket and does **not** use `MsgStore`.

The client is the existing `useJsonPatchWsStream`
(`packages/web-core/src/shared/hooks/useJsonPatchWsStream.ts:33`) — no new
frontend transport code.

### Message sequence

1. **On accept** — a resnapshot: `replace /nodes` plus `replace /generated_at`.
2. **Every tick (2s)** — per node that returned samples:
   - `replace /nodes/{node_id}/latest`
   - `add /nodes/{node_id}/history/-` — **once per sample received**
   - `remove /nodes/{node_id}/history/0` — once per append, after the window is
     full

   A node that returned nothing this tick gets **no operations at all** — not an
   empty append, not a redundant `replace` of unchanged data.
3. **On a transition** — `replace /nodes/{node_id}/availability` or
   `replace /nodes/{node_id}/health`.
4. **Resnapshot** on:
   - every 30s, unconditionally — the convergence backstop;
   - a replay gap on any node's cursor;
   - any change to the node set;
   - a batch large enough that appending it individually would be worse than
     resending (e.g. the first poll after connect, which returns the whole ring).

**The resnapshot targets `/nodes` and `/generated_at`, never the document
root.** A `replace` at path `""` cannot be applied by the consuming hook:
`useJsonPatchWsStream` mutates an Immer draft, and rfc6902 evaluates an empty
pointer to `parent === null`, so the op fails and `applyUpsertPatch`'s `add`
retry attempts `null[''] = value`. The backstop would silently never land, which
would take the entire convergence story with it. The repo's own approvals stream
uses a named sub-path for the same reason
(`crates/services/src/services/events/patches.rs:160-165`). The client seeds
`initialData()` with `{ nodes: {}, generated_at: null, sample_interval_ms: 2000 }`
so both paths exist before the first patch arrives. See analysis E4.

Point 4 is the level-triggered rule: patches are the optimisation, the snapshot
is the truth. A dropped broadcast message is otherwise undetectable by the
client, and `wiki/electric-sync-fallback.md` plus `wiki/self-hosted-deployment.md`
both record edge-triggered-only designs stalling silently in this codebase.

`history` is the only array patched positionally, and only at its two ends
(`-` and `0`), which is order-stable under append-and-evict. Every other
collection is keyed.

### Lifecycle

| Event | Effect |
| --- | --- |
| WS accepted | `subscriber_count += 1`; the worker collector starts if it was 0 |
| WS closed (clean **or** abnormal) | `subscriber_count -= 1`; the collector stops at 0 |

FR-16 is satisfied structurally: with no subscribers the coordinator issues zero
`GET /v1/metrics` requests, and an idle cluster costs one local `/proc` read per
tick. The decrement must be on the close path itself, not on a graceful-shutdown
branch, or an abnormally closed socket leaks a subscriber and the collector runs
forever.

**Close detection alone is not sufficient.** A half-open TCP connection — a
closed laptop lid, a dropped tunnel — never delivers a close, so the socket's
receive loop simply blocks; meanwhile the client reconnects and increments a
*new* subscriber. FR-16 would then fail open indefinitely. The handler therefore
sends a WebSocket ping on a fixed interval and drops the connection (releasing
its subscriber slot) if no pong arrives within a bounded deadline. See analysis
W5.

### Errors

- A failed upgrade is refused before the handler runs, by the existing signed-WS
  middleware.
- A per-node collection failure is **not** a stream error. It becomes that
  node's `availability` and is patched like any other field (FR-19).
- The client must handle `error` and premature `close`: a `WebSocket`
  constructor can resolve before its HTTP upgrade is rejected, so promise
  resolution alone is not evidence of a live socket, and the drawer would
  otherwise sit in "connecting" forever
  (`docs/knowledge-base/cli-tool-oauth-login.md`).
- A transparent reconnect-and-resnapshot is **recovery, not an error**, and must
  not raise a user-facing banner (`wiki/electric-sync-fallback.md`).
- Once any data has arrived, `useJsonPatchWsStream` reports no further errors
  (`useJsonPatchWsStream.ts:170-195` — it sets `error` only while
  `!dataRef.current`). A later interruption therefore surfaces as **per-node
  staleness** (FR-18), not a view-level error. FR-20a scopes the requirement to
  match; changing the hook would put approvals, raw logs, normalized logs,
  scratch, and browser sessions in the blast radius. See analysis W4.

---

## Generated types

`ClusterMetricsSnapshot`, `MetricsNode`, `NodeRole`,
`NodeMetricsAvailability`, `HostSample`, `CpuSample`, `MemorySample`,
`FilesystemSample`, `NetworkSample`, `ProcessSample`, and `SampleBatch` derive
`TS` and are registered in `crates/server/src/bin/generate_types.rs:75-77`
beside `WorkerNode`. Regenerate with `pnpm run generate-types`;
`shared/types.ts` is never hand-edited.

`crates/cluster-protocol` is **not** given a `ts-rs` dependency, and
`WorkerNode.resource_snapshot`'s `#[ts(type = "unknown")]` is left alone —
changing either would widen the blast radius into scheduling for no benefit
here.

---

## Guarantees this API makes

1. It is **read-only**. There is no mutating verb, and no field the client can
   send that influences what a host reads (FR-23, FR-29).
2. It **never** writes `worker_nodes.status`, a lease, or eligibility, and no
   scheduling decision reads from it (FR-21, FR-22).
3. Absence is **typed** — `unreachable`, `unsupported`, `not_implemented` — and
   never rendered as a zero reading (FR-17).
4. Command lines arrive **pre-redacted** from the node that read them; this
   layer neither unmasks nor needs to re-mask (FR-26).
5. Nothing is persisted. No table, no migration, no retention across restart.
