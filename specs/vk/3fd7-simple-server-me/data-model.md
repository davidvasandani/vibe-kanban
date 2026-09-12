# Data Model: Cluster Server Metrics (`3fd7-simple-server-me`)

All types live in `crates/node-metrics/src/types.rs` unless noted, derive
`Debug, Clone, Serialize, Deserialize, TS`, and are registered in
`crates/server/src/bin/generate_types.rs`. **Nothing here is persisted** — there
is no migration, no table, and no column. The entire model lives in memory on
the coordinator and on each worker.

## Invariants

1. **No `f32`/`f64` inside an internally-tagged enum.** `serde_json` is built
   with `preserve_order` (`Cargo.toml:44`), under which such a field fails to
   deserialize. Every percentage and load average lives in a plain struct.
   Guarded by a round-trip test over `NodeMetricsAvailability`.
2. **Absent ≠ zero.** Any reading the host could not supply, and any rate with
   no predecessor to derive against, is `None` — never `0`. This is why nearly
   every numeric field below is `Option`: a non-`Option` field is a promise that
   the value is always knowable, and for host introspection that promise is
   almost never true.
3. **Collections are keyed by stable identity.** Nodes by `node_id`, processes
   by `(pid, start_ticks)`, filesystems by `mount_point`, interfaces by
   `interface`. Never by array position.
4. **`sequence` is monotonic per sampler** and never reused.
5. **Redaction happens before construction.** A `ProcessSample` cannot hold an
   unredacted command; the redactor runs inside the collector.
6. **All enums are `rename_all = "snake_case"`.** Fixed explicitly on every
   enum, because a casing mismatch between backend and frontend fails silently
   as a comparison that is never true.

---

## `HostSample`

One observation of one host at one instant.

| Field | Type | Notes |
| --- | --- | --- |
| `sequence` | `u64` | Monotonic per sampler; the cursor consumers advance |
| `hostname` | `String` | Read at sample time; the only source of a node's hostname (`ClusterConfig` has no such field) |
| `captured_at` | `DateTime<Utc>` | When the read completed |
| `interval_ms` | `Option<u64>` | Real elapsed time since the previous sample; `None` on the first |
| `uptime_seconds` | `Option<u64>` | `/proc/uptime` |
| `cpu` | `CpuSample` | |
| `memory` | `MemorySample` | |
| `filesystems` | `Option<Vec<FilesystemSample>>` | `None` if the mount table was unreadable — distinct from "no filesystems" |
| `networks` | `Option<Vec<NetworkSample>>` | `None` if `/proc/net/dev` was unreadable |
| `processes` | `Option<Vec<ProcessSample>>` | **Populated on the newest sample only** (C4). `None` on retained history — which is why it is `Option` and not an empty `Vec`: an empty list would be indistinguishable from "no processes readable" |
| `degraded` | `Vec<String>` | Human-readable notes about what could not be read |

`interval_ms` carries the *actual* elapsed time rather than the configured
interval, so a delayed tick produces a correctly-scaled rate and the UI can
label an irregular gap.

## `CpuSample`

| Field | Type | Source |
| --- | --- | --- |
| `model` | `Option<String>` | `/proc/cpuinfo` `model name` |
| `core_count` | `Option<u32>` | count of `cpuN` lines in `/proc/stat` |
| `total_busy_percent` | `Option<f32>` | `/proc/stat` `cpu` line, `1 − Δidle/Δtotal`. **`None` until a predecessor exists** (FR-7) |
| `per_core_busy_percent` | `Option<Vec<f32>>` | one per `cpuN`, ordered by index; `None` until a predecessor exists |
| `load_1m` / `load_5m` / `load_15m` | `Option<f32>` | `/proc/loadavg` |
| `frequency_mhz` | `Option<u32>` | mean `cpu MHz` from `/proc/cpuinfo` |
| `temperature_celsius` | `Option<f32>` | `/sys/class/thermal/thermal_zone*/temp`, preferring `x86_pkg_temp`/`coretemp` |

`per_core_busy_percent` is positional by core index — the one place an index is
meaningful, because core 3 is core 3. The vector is replaced wholesale, never
patched per element.

## `MemorySample`

All `Option<u64>`: an unreadable `/proc/meminfo` yields `None` throughout, not a
machine that appears to have zero memory.

| Field | Derivation from `/proc/meminfo` |
| --- | --- |
| `total_bytes` | `MemTotal` |
| `available_bytes` | `MemAvailable` |
| `used_bytes` | `total − available` |
| `cached_bytes` | `Cached + SReclaimable` |
| `swap_total_bytes` | `SwapTotal` |
| `swap_used_bytes` | `SwapTotal − SwapFree` |

`used = total − available` rather than `total − free` deliberately: `MemFree`
excludes reclaimable page cache and makes a healthy Linux box look full.

## `FilesystemSample`

Keyed by `mount_point`.

| Field | Type |
| --- | --- |
| `mount_point` | `String` |
| `device` | `String` |
| `fs_type` | `String` |
| `total_bytes` / `used_bytes` / `available_bytes` | `Option<u64>` (`None` if `statvfs` failed on an otherwise-listed mount) |

Sourced from `/proc/self/mounts` plus `statvfs`. Pseudo filesystems are skipped
(`proc`, `sysfs`, `devtmpfs`, `cgroup*`, `overlay`, `squashfs`, `nsfs`,
`autofs`, and `tmpfs` under `/run`), as are duplicate devices. **NFS mounts are
kept** — the shared root filling up is one of the main things this panel exists
to catch.

## `NetworkSample`

Keyed by `interface`.

| Field | Type | Notes |
| --- | --- | --- |
| `interface` | `String` | `lo` and never-used interfaces are skipped |
| `rx_bytes_total` / `tx_bytes_total` | `u64` | lifetime counters from `/proc/net/dev` |
| `rx_bytes_per_second` / `tx_bytes_per_second` | `Option<u64>` | Δbytes ÷ Δseconds |

The rate is `None`, not `0`, whenever it cannot be derived: on the first sample,
and when the counter has gone backwards (interface reset). In the reset case a
`degraded` note records it. A zero here would read as "no traffic", which is a
different and false claim.

## `ProcessSample`

Identity is `(pid, start_ticks)`. PIDs are reused; the pair is not.

| Field | Type | Source |
| --- | --- | --- |
| `pid` | `i32` | |
| `start_ticks` | `u64` | `/proc/[pid]/stat` field 22 — identity only, never displayed |
| `name` | `String` | `comm` |
| `user` | `Option<String>` | resolved from `Uid:` in `/proc/[pid]/status` |
| `command` | `String` | `/proc/[pid]/cmdline`, **redacted**, truncated to 256 chars |
| `cpu_percent` | `Option<f32>` | `Δ(utime+stime) / (ticks_per_second × Δs) × 100`, capped at `core_count × 100`; `None` for a process first seen this sample |
| `memory_bytes` | `Option<u64>` | `VmRSS` |
| `thread_count` | `Option<u32>` | `Threads:` |

Sorted by `cpu_percent` descending (`None` last), truncated to
`SamplerConfig.max_processes` (15). `/proc/[pid]/environ` is never opened.

## `SampleBatch`

The worker's response and the sampler's read interface.

| Field | Type | Notes |
| --- | --- | --- |
| `samples` | `Vec<HostSample>` | Oldest → newest, those with `sequence > after` |
| `earliest_retained_sequence` | `u64` | A cursor below this has fallen out of the ring |
| `latest_sequence` | `u64` | |

A gap is reported, not hidden. For metrics a gap is benign — a hole in a graph —
so the consumer records the discontinuity and forces a resnapshot rather than
erroring, unlike the execution journal where a gap is fatal.

**A batch may contain zero, one, or many samples.** The worker's sampler and the
coordinator's poller are independently phased and neither is synchronised to the
other, so jitter, a timed-out poll, or a catch-up routinely produce 0 or 2
samples for a nominal 2s tick, and the first poll after a subscriber connects
(`after=0`) returns the whole retained ring — up to 150. Consumers must handle
all three cases; see the patch rules below.

## `SamplerConfig`

| Field | Value | Fixed by |
| --- | --- | --- |
| `interval` | 2s | C3 — one constant, not operator-adjustable |
| `retention` | 150 samples ≈ 5 min | C4 |
| `max_processes` | 15 | C4 |

## `NodeRole`

`#[serde(rename_all = "snake_case")]` → `"coordinator"` | `"worker"`.
Externally tagged (a plain string), so the `preserve_order` hazard does not
apply.

## `NodeMetricsAvailability`

Internally tagged (`#[serde(tag = "status", rename_all = "snake_case")]`),
mirroring `MountHealth` in `crates/cluster-protocol/src/lib.rs:54-63`.
**Float-free by invariant 1.**

| Variant | Fields | Meaning |
| --- | --- | --- |
| `Available` | — | Reporting normally |
| `Stale` | `since: DateTime<Utc>` | Last contact succeeded, but no fresh sample since `since`. Readings are retained and rendered de-emphasised (FR-18) |
| `NotCollected` | — | No poll has been attempted — the normal state of a worker when no subscriber has been connected. Distinct from a failure |
| `Unsupported` | `platform: String` | Not a Linux host |
| `Unreachable` | `reason: String` | Transport, auth, timeout, or oversized reply this cycle |
| `NotImplemented` | — | The worker build predates `GET /v1/metrics` |

Constitution XIX names stale as a status in its own right, and it is: inferring
staleness on the client from `last_contact_at` arithmetic is exactly the
fabricated derivation the principle is written against.

None of these variants is load-bearing. `Unreachable` never causes
`WorkerNode::mark_offline`, never touches a lease, and never affects
`scheduler::eligibility`.

## `NodeHealth`

Distinct from `NodeMetricsAvailability`, and this distinction is the point:
*availability* is whether we could read metrics, *health* is the cluster's own
judgement of the node. FR-24 requires the drawer to agree with Settings, so the
drawer must carry the same health the rest of the product uses.

| Field | Type | Source |
| --- | --- | --- |
| `status` | `WorkerNodeStatus` | `worker_nodes.status`, **adjusted read-only** for an expired lease |
| `mount_status` | `Option<WorkerMountStatus>` | `worker_nodes.mount_status`; `None` for the coordinator |
| `lease_expires_at` | `Option<DateTime<Utc>>` | |
| `schedulable` | `bool` | `status == online && mount_status == healthy`, matching `WorkersSettingsSection`'s existing derivation |

**The lease adjustment is computed in memory, never written.** A row whose
`lease_expires_at <= now` is *displayed* as `offline` regardless of its stored
`status`. Calling `WorkerRegistry::expire_heartbeats` here — as `list_workers`
does — would issue `UPDATE worker_nodes SET status = 'offline'`
(`crates/db/src/models/worker_node.rs:160-173`) from a monitoring path, making a
real lifecycle transition depend on whether an operator has a drawer open. That
is precisely what Constitution XIX and FR-21 forbid. See analysis finding E1.

---

## Coordinator aggregate

In `crates/services/src/services/cluster/metrics.rs`.

### `MetricsNode`

| Field | Type | Notes |
| --- | --- | --- |
| `node_id` | `Uuid` | See identity rules below |
| `hostname` | `String` | From `HostSample.hostname`, falling back to `worker_nodes.hostname` |
| `role` | `NodeRole` | |
| `health` | `Option<NodeHealth>` | The cluster's judgement; `None` for the coordinator, which has no worker row |
| `availability` | `NodeMetricsAvailability` | Whether metrics could be read |
| `latest` | `Option<HostSample>` | The only sample carrying `processes` |
| `history` | `Vec<HostSample>` | Bounded; `processes` is `None` on every entry |
| `last_contact_at` | `Option<DateTime<Utc>>` | When this node last reported |

**Every worker row is listed, including `Offline` ones.** Collection skips them
— there is no point polling a node we believe is down — but omitting them from
the list would leave a lease-expired worker frozen at its last `availability`,
plausibly `available`, which is the exact "healthy here, dead in Settings"
inversion FR-24 forbids. A non-polled node gets `availability: NotCollected` and
its real `health`.

When `availability` is not `Available` and `latest` is older than the retention
window, `latest` and `history` are cleared (C5) and only `availability`,
`health`, and `last_contact_at` remain — five-minute-old numbers are not
evidence.

#### Node identity

| Node | `node_id` |
| --- | --- |
| Worker | `worker_nodes.id` |
| Coordinator, clustering enabled | `ClusterConfig.coordinator_id` |
| Coordinator, clustering disabled | `Uuid::new_v5(&COORDINATOR_NAMESPACE, hostname.as_bytes())` |

`ClusterConfig.coordinator_id` is `Option<Uuid>` and defaults to `None`
(`crates/services/src/services/cluster/config.rs:41, :53`) — it is only required
when clustering is enabled. The v5 fallback is **stable across restarts**, which
a `Uuid::new_v4()` would not be; an unstable id would break the persisted
`selectedNodeId` (FR-13) on every restart and make the first acceptance
criterion untestable.

### `ClusterMetricsSnapshot`

| Field | Type | Notes |
| --- | --- | --- |
| `nodes` | `BTreeMap<Uuid, MetricsNode>` | **A map, not a `Vec`** — this is what makes node-keyed JSON patches possible |
| `generated_at` | `DateTime<Utc>` | |
| `sample_interval_ms` | `u64` | So the client can size its sparkline x-axis without hardcoding the cadence |

The coordinator's entry is synthesised each time the snapshot is built, from
`ClusterConfig` plus the local sampler. It is never written to `worker_nodes`
(FR-3), which keeps it invisible to `scheduler::eligibility()` and to
`WorkerNode::fetch_all`.

## JSON Patch paths

| Operation | Path | When |
| --- | --- | --- |
| `replace` | `/nodes` **and** `/generated_at` | Resnapshot: on connect, every 30s, on replay gap, on membership change |
| `replace` | `/nodes/{node_id}/latest` | Per tick, if ≥1 sample was received for that node |
| `add` | `/nodes/{node_id}/history/-` | **Once per sample received** |
| `remove` | `/nodes/{node_id}/history/0` | Once per append, after the window is full |
| `replace` | `/nodes/{node_id}/availability` | On an availability transition |
| `replace` | `/nodes/{node_id}/health` | On a health transition |

**The resnapshot targets `/nodes`, not the document root.** A `replace` at path
`""` cannot be applied by the consuming hook: `useJsonPatchWsStream`
(`packages/web-core/src/shared/hooks/useJsonPatchWsStream.ts:33`) mutates an
Immer draft, and for an empty pointer rfc6902 yields `parent === null`, so the
op fails and `applyUpsertPatch`'s `add` retry then attempts `null[''] = value`.
The snapshot would never land. The repo's own approvals stream uses a named
sub-path for exactly this reason
(`crates/services/src/services/events/patches.rs:160-165`). The client seeds
`initialData()` with `{ nodes: {}, generated_at: null, sample_interval_ms: 2000 }`
so both paths exist before the first patch. See analysis finding E4.

**A tick may carry any number of samples.** The builder emits one
`add /history/-` (and one matching `remove /history/0` once full) **per sample
received**, not one per tick. A tick that received nothing for a node emits
nothing for that node — not an empty append, and not a `replace` of `latest`
with unchanged data. A first poll returning 150 samples resnapshots instead of
emitting 150 appends.

`history` is the one array patched positionally, and only ever at its two ends
(`-` and `0`), which is order-stable under append-and-evict. Every other
collection is addressed by key.
