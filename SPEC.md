# Technical Spec: Cluster Server Metrics (btop-style right drawer)

Task id: `3fd7-simple-server-me`

> Constraints distilled from the project knowledge base are in
> [`PRIOR_KNOWLEDGE.md`](PRIOR_KNOWLEDGE.md); the load-bearing ones are folded
> into the design sections below and cited where they apply.

## Summary

Add live, btop-style host metrics for every machine in the Vibe Kanban cluster,
surfaced in a right-side drawer that can be opened from anywhere in the app.

Each node — the coordinator and every registered worker — runs a bounded,
cursor-addressed sampler that reads host counters (`/proc`, `/sys`) on a fixed
cadence and retains a short rolling window. The coordinator aggregates its own
samples with samples pulled from each worker over the existing signed
coordinator→worker channel, and streams the merged view to the browser.

The drawer renders one node at a time (with a node switcher and an all-node
overview strip): CPU total plus per-core meters, memory and swap, filesystem
usage, per-interface network throughput, and a top-N process table.

Metrics are strictly observational. Nothing in this feature schedules work,
changes worker eligibility, marks a node offline, or acts on a process.

## Motivation

The cluster (`think2` coordinator; `think3`, `think4` workers) currently exposes
only four scalars per worker — `cpu_count`, `load_1m`, `available_memory_bytes`,
`active_execution_count` — stored as an untyped JSON blob on `worker_nodes` and
rendered as two numbers in Settings → Workers. Diagnosing "why is this node
slow / out of disk / saturated" means SSHing in and running `btop`. The
coordinator's own host has no in-app visibility at all.

## Scope

- Vibe Kanban source in this repository.
- A new `crates/node-metrics` crate: host counter sampling and derivation.
- Worker: a background sampler and a new signed `GET /v1/metrics` route.
- Coordinator: a local sampler, a subscriber-gated worker collector, an
  in-memory aggregate store, a REST snapshot endpoint, and a WebSocket stream.
- Frontend: a right-anchored overlay drawer, its toggle, and the metric panels.
- Generated TypeScript types for the new metric shapes.
- Documentation and knowledge-base updates.

## Out of Scope

- Changes to services other than Vibe Kanban.
- Any deployment change. This feature introduces no new port, no new unit, and
  no new Nix option; it rides the existing coordinator↔worker channel, so
  `modules/vibe-kanban-rebuild.nix` in the homelab repo is untouched.
- Process control (kill, terminate, renice) or any other write action on a host.
- Long-term metric storage, on-disk persistence, alerting, or thresholds that
  trigger behaviour.
- Changing worker scheduling, eligibility, lease, or health semantics.
- GPU, per-process disk I/O, and container/cgroup breakdowns.
- macOS/Windows collection. Non-Linux nodes report `unsupported` and the UI
  says so (the cluster is all NixOS; the macmini is a client, not a node).

## Background: what already exists

| Concern | Where |
| --- | --- |
| Worker→coordinator heartbeat (push, 10s, coordinator-dictated) | `crates/worker/src/server.rs:98-170`, `crates/services/src/services/cluster/registry.rs:95-130` |
| Coordinator→worker signed request/response client | `crates/services/src/services/cluster/client.rs` (`inventory()` at `:279` is the closest template) |
| Worker inbound router + `require_signature` | `crates/worker/src/worker_api.rs:89-114`, `:409-463` |
| Coordinator admin routes for nodes | `crates/server/src/routes/workers.rs:33-37` |
| Existing scalar snapshot | `crates/cluster-protocol/src/lib.rs:46-52`, collected at `crates/worker/src/server.rs:320-347` |
| Worker rows + untyped `resource_snapshot` column | `crates/db/src/models/worker_node.rs`, migration `20260731000000_cluster_worker_persistence.sql` |
| Snapshot-then-JSON-Patch WS pattern | `crates/server/src/routes/approvals.rs:51-112`, `crates/services/src/services/approvals.rs:233`, client `packages/web-core/src/shared/hooks/useJsonPatchWsStream.ts:33` |
| Left-anchored portal drawer to mirror | `packages/ui/src/components/MobileDrawer.tsx:12` |
| Hand-rolled SVG gauge idiom (no chart library exists) | `packages/ui/src/components/ContextUsageGauge.tsx` |
| Existing worker UI + query key | `packages/web-core/src/shared/dialogs/settings/settings/WorkersSettingsSection.tsx` |

## Design

### 1. Node identity

A *node* is a host that can report metrics. There are two kinds:

- **Coordinator** — exactly one, always present, even when clustering is
  disabled. It has no `worker_nodes` row and must not get one: `worker_nodes`
  carries `UNIQUE(hostname)` and a synthetic row would collide with a real
  worker on the same host and pollute scheduling. The coordinator node is
  synthesised at the API layer, with `node_id = cluster_config.coordinator_id`
  when set, and a stable locally-derived UUID otherwise.
- **Worker** — one per row in `worker_nodes`, `node_id = worker_nodes.id`.

```rust
pub struct MetricsNode {
    pub node_id: Uuid,
    pub hostname: String,
    pub role: NodeRole,               // Coordinator | Worker
    pub availability: NodeMetricsAvailability,
    pub samples: Vec<HostSample>,     // oldest → newest, bounded
}

pub enum NodeMetricsAvailability {
    Available,
    Unsupported { platform: String },   // non-Linux node
    Unreachable { reason: String },     // transport/auth failure this cycle
    NotImplemented,                     // worker predates GET /v1/metrics
}
```

`NodeMetricsAvailability` is internally tagged (matching `MountHealth`) and must
stay **float-free**. `serde_json` is built with `preserve_order` workspace-wide
(`Cargo.toml:44`), and deserializing an internally-tagged enum whose variant
carries an `f32`/`f64` field fails with `invalid type: map, expected f64`.
Every percentage and load average therefore lives in a plain struct, never in a
tagged variant.

**Availability is descriptive, never load-bearing.** `Unreachable` must not call
`WorkerNode::mark_offline`, must not touch the lease, and must not affect
`scheduler::eligibility`. A node absent from metrics is still schedulable; a
node present in metrics is not thereby healthy. This keeps the feature inside
Constitution XVIII: liveness and terminal state still require the existing
worker evidence, and a metrics timeout is not evidence of anything.

### 2. Sampling: `crates/node-metrics`

A new leaf crate depending only on `serde`, `chrono`, `uuid`, `ts-rs`, and
`thiserror` (all existing workspace dependencies). **No new third-party
dependency.** `sysinfo` was considered and rejected in the plan's research
notes: the workspace already hand-parses `/proc`
(`crates/worker/src/server.rs:320`), the fleet is Linux-only, and pure
`&str → struct` parsers are unit-testable against checked-in fixtures without
touching the host — which `sysinfo` is not.

Public surface:

```rust
pub struct MetricsSampler { /* ring buffer + previous raw counters */ }

impl MetricsSampler {
    pub fn new(config: SamplerConfig) -> Self;
    /// Read the host once, derive rates against the previous read, append.
    pub fn sample_now(&self) -> Result<(), SampleError>;
    /// Samples with `sequence > after`, oldest first, plus retention info.
    pub fn since(&self, after: u64) -> SampleBatch;
    pub fn spawn(self: Arc<Self>, shutdown: CancellationToken) -> JoinHandle<()>;
}

pub struct SamplerConfig {
    pub interval: Duration,   // fixed 2s (btop's default refresh) — see clarification C3
    pub retention: usize,     // 150 samples ≈ 5 minutes — see clarification C4
    pub max_processes: usize, // 15
}
```

**The ring does not retain process tables** (clarification C4). The process
table dominates a sample — ~3 KB for 15 rows against well under 1 KB for
everything else — and nothing plots processes over time. The ring therefore
holds `HostSample` minus `processes`, and the process table is carried on the
*latest* sample only. This takes per-node retention from ~450 KB to ~150 KB and
keeps the streamed patch payload small, which is what the bounded-stream rule
exists to protect. Concretely: `HostSample.processes` is populated only on the
newest entry, and `SampleBatch` carries the current table once rather than per
sample.

`SampleBatch` mirrors the execution journal's replay contract:

```rust
pub struct SampleBatch {
    pub samples: Vec<HostSample>,
    pub earliest_retained_sequence: u64,
    pub latest_sequence: u64,
}
```

A caller whose cursor is older than `earliest_retained_sequence` has fallen out
of the ring; the batch is still returned but the gap is explicit, exactly as
`WorkerClientError::ReplayGap` makes journal gaps explicit. For metrics a gap is
benign (a hole in a graph), so the consumer records the discontinuity rather
than erroring.

`HostSample` (fields are `Option` wherever the source may be absent, so a
missing `/sys` file degrades one row rather than the whole sample):

```rust
pub struct HostSample {
    pub sequence: u64,
    pub captured_at: DateTime<Utc>,
    pub interval_ms: u64,          // actual elapsed since previous sample
    pub uptime_seconds: Option<u64>,
    pub cpu: CpuSample,
    pub memory: MemorySample,
    pub filesystems: Vec<FilesystemSample>,
    pub networks: Vec<NetworkSample>,
    pub processes: Vec<ProcessSample>,
    pub degraded: Vec<String>,     // human-readable "couldn't read X" notes
}

pub struct CpuSample {
    pub model: Option<String>,
    pub core_count: u32,
    pub total_busy_percent: f32,          // 0..100
    pub per_core_busy_percent: Vec<f32>,
    pub load_1m: f32, pub load_5m: f32, pub load_15m: f32,
    pub frequency_mhz: Option<u32>,
    pub temperature_celsius: Option<f32>,
}

pub struct MemorySample {
    pub total_bytes: u64, pub available_bytes: u64,
    pub used_bytes: u64,  pub cached_bytes: u64,
    pub swap_total_bytes: u64, pub swap_used_bytes: u64,
}

pub struct FilesystemSample {
    pub mount_point: String, pub device: String, pub fs_type: String,
    pub total_bytes: u64, pub used_bytes: u64, pub available_bytes: u64,
}

pub struct NetworkSample {
    pub interface: String,
    pub rx_bytes_total: u64, pub tx_bytes_total: u64,
    pub rx_bytes_per_second: u64, pub tx_bytes_per_second: u64,
}

pub struct ProcessSample {
    pub pid: i32,
    pub start_ticks: u64,          // /proc/[pid]/stat field 22; identity, not display
    pub name: String, pub user: Option<String>,
    pub command: String,           // redacted; see §6
    pub cpu_percent: f32, pub memory_bytes: u64, pub thread_count: u32,
}
```

A process row's identity is `(pid, start_ticks)`, not its position in the array.
PIDs are reused, and a stale index causes a `replace` that overwrites whatever
row got reallocated at that position — the exact class of bug the
log-normalization knowledge-base page records being caught in review. The
previous-sample CPU-delta map and any patch path key on the pair.

Sources and derivations:

| Field | Source | Derivation |
| --- | --- | --- |
| per-core + total CPU busy | `/proc/stat` | `1 - Δidle/Δtotal` per `cpuN` line, against the previous raw read |
| load averages | `/proc/loadavg` | direct |
| CPU model / frequency | `/proc/cpuinfo` | `model name`, mean `cpu MHz` |
| CPU temperature | `/sys/class/thermal/thermal_zone*/temp` | first zone of type `x86_pkg_temp`/`coretemp`, else first zone; `None` on failure |
| uptime | `/proc/uptime` | first field |
| memory / swap | `/proc/meminfo` | `MemTotal`, `MemAvailable`, `Cached`+`SReclaimable`, `SwapTotal`−`SwapFree`; `used = total − available` |
| filesystems | `/proc/self/mounts` + `statvfs` | skip pseudo filesystems (`proc`, `sysfs`, `tmpfs` on `/run`, `devtmpfs`, `cgroup*`, `overlay`, `squashfs`, `nsfs`, `autofs`) and duplicate devices; **keep** NFS mounts — the shared root is exactly what an operator needs to see |
| networks | `/proc/net/dev` | Δbytes / Δseconds; skip `lo` and interfaces with zero lifetime traffic |
| processes | `/proc/[pid]/{stat,status,cmdline}` | `cpu_percent = Δ(utime+stime) / (ticks_per_second × Δseconds) × 100`, capped at `core_count × 100`; `memory_bytes` from `VmRSS`; sort by CPU desc, take `max_processes` |

The first sample after start has no predecessor, so all rate-derived fields
(per-core CPU, per-process CPU, network rates) are `0.0` and the sample is
tagged `degraded: ["first sample: rates not yet available"]`. The sampler must
not publish a rate computed against a zero baseline.

A monotonic counter that goes backwards (interface reset, PID reuse, `/proc/stat`
after a suspend) yields `0` for that field rather than a negative or a wildly
large rate, and adds a `degraded` note.

Sampling runs on `tokio::task::spawn_blocking` — the current
`resource_snapshot()` does blocking `std::fs` reads inline in the async
heartbeat path, and a full process walk is materially more work.

The sampler stores raw previous counters, not derived values, so a skipped or
delayed tick yields a correct (longer-interval) rate rather than a spike.
`interval_ms` carries the real elapsed time so the UI can label irregular gaps.

Every parser is a free function over `&str` (`parse_proc_stat`, `parse_meminfo`,
`parse_net_dev`, `parse_loadavg`, `parse_process_stat`, …) with `#[cfg(test)]`
coverage against fixtures captured from a real NixOS host, including truncated,
empty, and unexpected-column inputs.

Two filesystem traps the knowledge base records, which the `/proc` walk hits
directly:

- **`read_dir(..).filter_map(|e| e.ok())` silently drops unreadable entries.**
  A `/proc/[pid]` directory that vanished mid-walk (process exited — expected,
  skip silently) and one that could not be read (permissions — unexpected) are
  different facts. The walk distinguishes `ErrorKind::NotFound` from every other
  error, and the latter increments a counter surfaced in `degraded`.
- **`Path::exists()` returns `false` for both "absent" and "stat failed"** — use
  `try_exists()` wherever presence is being decided.

An errored read produces a `None`/`degraded` entry, never a zero.

### 3. Worker side

- `crates/worker/src/lib.rs::run()` constructs `Arc<MetricsSampler>` and spawns
  its ticker alongside the existing registration loop, with the same shutdown
  token as the rest of the worker.
- New route in `crates/worker/src/worker_api.rs`:
  `GET /v1/metrics?after={u64}` → `SampleBatch`.
  It is added inside the router that already carries `require_signature`, so it
  inherits the ed25519 transport signature, the ±30s drift window, and the body
  digest check. Like `GET /v1/jobs` and terminal output, it carries no
  payload-level `RequestAuthority` (there is no body to bind), and the signed
  target includes the query string — which is what stops an attacker replaying
  a signature against a different cursor.
- `ResourceSnapshot`, `WorkerHeartbeat`, and `PROTOCOL_VERSION` are **not
  changed**. The heartbeat keeps carrying the four scheduler scalars under their
  existing key names; `scheduler::score()` reads `load_1m` and
  `active_execution_count` out of `resource_snapshot` by string key, and
  renaming or nesting them silently makes every worker score `f64::INFINITY`.
  Metrics ride an entirely separate pull channel.

### 4. Coordinator side

New `crates/services/src/services/cluster/metrics.rs`, exported from the
existing `cluster` barrel:

```rust
pub struct ClusterMetricsService { /* local sampler, per-node rings, subscribers */ }
```

- **Local sampler** — a `MetricsSampler` for the coordinator host, started with
  the service and always running (one `/proc` read every 2s is negligible, and
  it means the drawer has history the moment it opens).
- **Worker collector** — a task that, **only while `subscriber_count > 0`**,
  polls every non-`Offline` worker concurrently each tick via a new
  `WorkerClient::metrics(worker_node_id, after)` method placed beside
  `inventory()`. Per-node cursor state lets each poll return only new samples.
  Three lifetime rules from the knowledge base apply verbatim:
  - the task holds only a **`Weak`** to the service between ticks — a strong
    clone in the loop leaks the service forever;
  - it re-checks the subscriber count **every tick** and exits at zero, because
    dropping the consumer does not by itself stop a spawned loop;
  - it never holds the node-map lock across an `await`; it collects targets
    under the lock, drops it, polls, then writes back only if the node's
    generation is unchanged (a worker can deregister and re-register in the
    window).
- **Node list freshness** — the collector and the REST/WS handlers call
  `WorkerRegistry::expire_heartbeats(now)` before listing, exactly as
  `list_workers` does. Filtering stale leases only inside scheduler selection
  leaves an admin surface claiming a dead worker is healthy.
- **Signing per poll** — every poll builds a **fresh timestamp and nonce**. A
  cached signed envelope is a replay and will be rejected; the signed target
  includes `?after=N`, so a cursor cannot be substituted.
- **Response cap** — the coordinator caps the metrics response body before
  buffering it (per-core plus per-process arrays are not small), and rejects an
  oversized reply as `Unreachable { reason }` for that node only.
- **Failure isolation** — each worker is polled independently; one failure
  produces `Unreachable { reason }` for that node only. HTTP 404 (a worker that
  predates this feature) maps to `NotImplemented`, is logged once per node at
  `debug!`, and is not retried more often than the normal tick. No failure path
  touches worker status, lease, or eligibility.
- **Backpressure and cost** — the collector tick equals the sample interval
  (2s). With zero subscribers it does nothing, so an idle cluster pays only the
  local `/proc` read. `WorkerClient`'s existing 30s reqwest timeout is too long
  for a 2s cadence; the metrics call uses a per-request 5s timeout so a hung
  node cannot stall the tick, and one in-flight request per node is enforced.

Exposure, in `crates/server/src/routes/workers.rs` (or a sibling
`cluster_metrics.rs` mounted from the same `admin_router()`):

| Method | Path | Returns |
| --- | --- | --- |
| GET | `/api/cluster/metrics` | `ApiResponse<ClusterMetricsSnapshot>` — full current window for every node |
| GET | `/api/cluster/metrics/ws` | snapshot-then-JSON-Patch stream |

The WS endpoint uses `SignedWsUpgrade`
(`crates/server/src/middleware/signed_ws.rs:33`) and the
`MsgStore`/`LogMsg::JsonPatch` idiom already used by
`/api/approvals/stream/ws`, so the client is the existing
`useJsonPatchWsStream`. On connect the server emits one `replace` patch at the
document root carrying the whole snapshot; each subsequent tick emits, per node,
a `replace` of `/nodes/{node_id}/latest`, an `add` of
`/nodes/{node_id}/history/-`, and a `remove` of `/nodes/{node_id}/history/0`
once the window is full.

**Nodes are keyed by `node_id` in an object, never positioned in an array.**
Array-index patches break the moment a worker registers, drains, or is removed
mid-stream.

**A periodic full resnapshot is the convergence backstop.** A pure patch stream
stalls silently — broadcast lag can drop a message, and a client that missed one
has no way to know. Every 30s, and whenever a node's cursor shows a replay gap
or the node set changes, the server emits a root `replace` instead of a delta.
This is the same level-triggered-over-edge-triggered rule the deploy reconciler
follows: patches are the optimisation, the snapshot is the truth.

**Nothing is persisted.** Samples live only in memory. SQLite is single-writer
in this deployment and per-2-second data has no business in it.

Subscriber counting drives the collector: the count increments on WS accept and
decrements on close (including abnormal close), and the collector stops when it
reaches zero. The REST endpoint serves whatever is currently in the rings
without starting the collector, so a snapshot request on an idle cluster returns
coordinator data plus stale-or-`Unreachable` worker data rather than blocking.

### 5. Types

`HostSample` and its children derive `TS` in `crates/node-metrics` and are
registered in `crates/server/src/bin/generate_types.rs` beside the existing
`WorkerNode`/`WorkerNodeStatus` declarations, then regenerated with
`pnpm run generate-types`. `shared/types.ts` is never hand-edited.

This deliberately does **not** retrofit `ts-rs` onto `crates/cluster-protocol`
(which today derives only `Serialize`/`Deserialize`) or change
`WorkerNode.resource_snapshot`'s `#[ts(type = "unknown")]`. Those are separate
concerns and changing them would widen the blast radius into scheduling.

### 6. Redaction (security)

A process table is the one place this feature can leak secrets. Constitution
XVII requires that API results never expose environment values, tokens,
authorization material, authenticated URLs, or secret-bearing command
arguments. Therefore:

- **Environment variables are never read.** `/proc/[pid]/environ` is not opened.
- `/proc/[pid]/cmdline` is read, then passed through a redactor in
  `crates/node-metrics/src/redact.rs` before it ever leaves the sampler —
  redaction happens at the source, so an unredacted command never crosses the
  wire, never lands in the coordinator's ring, and never reaches a log line.
- The redactor masks, replacing the value with `«redacted»`:
  - the value of any argument whose key matches (case-insensitive)
    `token|secret|password|passwd|pwd|api[-_]?key|auth|credential|bearer|private[-_]?key|session`,
    in both `--key=value` and `--key value` forms;
  - any bare argument that looks like credential material: a run of ≥ 20
    characters from `[A-Za-z0-9+/=_-]` containing at least one digit and one
    letter, or the userinfo of a `scheme://user:password@host` URL;
  - any argument matching a known token prefix (`sk-`, `ghp_`, `gho_`,
    `github_pat_`, `xoxb-`, `xoxp-`, `op://`, `AKIA`, `eyJ` JWT-shaped values).
- The result is truncated to 256 characters with a trailing `…`.
- The redactor is unit-tested with a fixture list of realistic command lines,
  and the tests assert that no fixture's secret substring survives.

This is deliberately conservative in the "redact too much" direction: an
over-redacted command is a cosmetic loss, an under-redacted one is a leak.

### 7. Frontend

**Store** — `packages/web-core/src/shared/stores/useMetricsDrawerStore.ts`,
zustand + `persist` (key `metrics-drawer`), holding
`{ open, width, selectedNodeId, expandedPanels }`. A standalone persisted store,
modelled on `useOrgRailStore`, deliberately avoids the `useUiPreferencesStore` ↔
scratch ↔ Rust `UiPreferencesData` round-trip for what is purely local chrome.

**Toggle** — a new `ToggleServerMetrics` entry in the actions registry
(`packages/web-core/src/shared/actions/index.ts`, beside `ToggleRightSidebar`)
added to `NavbarActionGroups.right`. Unlike the sidebar toggles it is **not**
gated on `layoutMode === 'workspaces'` — cluster health is global. Icon:
`PulseIcon` (Phosphor).

**Drawer** — `packages/ui/src/components/MetricsDrawer.tsx`, presentational,
`createPortal` to `document.body`, right-anchored, mirroring `MobileDrawer`
(`right-0`, `translate-x-full` when closed, `transition-transform duration-200
ease-out`, backdrop `bg-black/50`). Default width 420px, drag-resizable between
360px and 720px, width persisted. Closes on `Escape` and backdrop click; focus
is trapped while open and restored to the toggle on close.

**Container** —
`packages/web-core/src/shared/components/ui-new/containers/ServerMetricsDrawerContainer.tsx`.
Subscribes with `useJsonPatchWsStream` **only while the drawer is open**, so no
socket and no coordinator collector runs when nobody is looking. Falls back to
`useQuery` against `GET /api/cluster/metrics` if the socket errors, mirroring
the Electric-then-REST fallback convention already used in this codebase.

Frontend rules carried over from the knowledge base:

- **One multiplexed socket for all nodes**, never one per node or per sparkline.
- **Host-scoped**, via `makeHostAwareRequest` / the selected machine client, and
  the query key includes the host scope. An unscoped path silently reports the
  UI's own machine for every node. A response that resolves after the selected
  host changed is discarded by a `useRef` generation guard.
- **The drawer's hooks are gated at a wrapper**, so a mobile viewport never
  mounts the subscription at all, rather than conditionally calling hooks.
- **Each node panel is wrapped in an error boundary** — one node with malformed
  data must not blank the drawer.
- **The scroll container carries `overflow-x-hidden` alongside `overflow-y-auto`**;
  `visible` on one axis combined with a scrolling value on the other computes to
  `auto`, which silently turns the drawer into a horizontal scroller.
- **`error` and premature `close` are handled explicitly.** A `WebSocket`
  constructor can succeed before its HTTP upgrade is rejected, so the drawer
  must not sit in "connecting" forever.
- **A transparent reconnect-and-resnapshot is recovery, not an error** — it must
  not raise a banner. Conversely, an error state does not clear itself: recovery
  emits an explicit clear that also resets the error-report debounce, or a fresh
  identical failure is debounced away.
- **Series colours are derived client-side**; no data field is added for
  colouring.
- Any debounced re-render compares content before re-arming its timer —
  re-arming in an effect cleanup starves the update under a 2 Hz patch stream.

**Panels** (stateless views, all in `packages/ui`):

| Panel | Content |
| --- | --- |
| Node strip | One compact row per node: hostname, role badge, CPU %, memory %, availability dot. Click selects. Always visible at the top. |
| CPU | Total busy meter + sparkline, per-core meter grid, model name, frequency, temperature, `load 1m/5m/15m`, uptime |
| Memory | Total / used / available / cached stacked meter + sparkline, swap meter |
| Disks | Per filesystem: mount point, used %, used/total, free — bar per row |
| Network | Per interface: ↓/↑ current rate + sparkline, lifetime totals |
| Processes | Top-N table: PID, name, user, threads, memory, CPU % — read-only, no actions |

Panels are collapsible sections following the `RightSidebar` /
`CollapsibleSectionHeader` idiom, expansion persisted in the drawer store.

**Drawing** — hand-rolled inline SVG (`Sparkline.tsx`) and Tailwind `w-[N%]`
meters, following `ContextUsageGauge`. **No charting library is added**; none
exists in the repo today and adding one for bars and polylines is not justified.

**Styling** — design tokens only, per `packages/local-web/AGENTS.md`:
`text-high` / `text-normal` / `text-low`, `bg-primary` / `bg-secondary` /
`bg-panel`, `border-border`, spacing `p-half` / `p-base`. Numeric readouts use
`font-ibm-plex-mono`. Metric severity uses the `ContextUsageGauge` bucket
convention (`text-low` → `text-normal` → `text-brand-secondary` → `text-error`);
btop's raw ANSI palette is not reproduced.

**Accessibility** — meters carry `role="img"` with an `aria-label` stating the
value; the drawer is `role="dialog"` with `aria-modal="false"` (it does not
block the app) and a labelled close control. All strings go through
`useTranslation` with inline English defaults.

**Degradation** — `Unsupported`, `Unreachable`, and `NotImplemented` render an
explicit per-node message with the last known sample de-emphasised and labelled
with the timestamp it was taken at — never a blank panel, and never a zero that
reads as a measurement.

Retained readings expire (clarification C5). Once a node's newest retained
sample is older than the retention window (5 minutes), the coordinator drops
that node's ring and the panel shows only the status and the time contact was
lost. Five-minute-old numbers are not evidence, and however greyed they are,
leaving them on screen invites them to be read as current.

## Acceptance criteria

1. With clustering disabled, opening the drawer shows exactly one node (the
   coordinator) with live CPU, memory, disk, network, and process panels.
2. With clustering enabled and `think3`/`think4` registered, the node strip
   lists three nodes and selecting each shows that host's own metrics.
3. Per-core CPU percentages track `btop` on the same host within a few points,
   and the first sample after start reports `0` rates with a `degraded` note
   rather than a spike.
4. Closing the drawer stops the WebSocket, and with no subscribers the
   coordinator issues no `GET /v1/metrics` requests to any worker.
5. Stopping `vibe-kanban-worker` on `think4` renders that node as `Unreachable`
   within one tick, while `think4`'s `worker_nodes.status`, lease, and
   scheduling eligibility are unchanged, and a workspace already placed there is
   unaffected.
6. A worker build without `GET /v1/metrics` renders as `NotImplemented`; the
   coordinator logs it at most once per node and keeps serving other nodes.
7. A command line containing `--api-key=sk-live-…`, a `https://u:p@host` URL, or
   a bare 40-character token appears in the process table with the secret
   replaced by `«redacted»`; `/proc/[pid]/environ` is never opened.
8. `GET /v1/metrics` without a valid signature, with a stale timestamp, or with
   a signature computed over a different `after` value is rejected `401`.
9. Reordering, adding, or removing a worker mid-stream does not corrupt the
   client's view (node-keyed patches, verified by a unit test over the patch
   builder).
10. Drawer open state, width, selected node, and panel expansion survive a page
    reload.
11. `cargo test --workspace`, `pnpm run check`, `pnpm run lint`, and
    `pnpm run generate-types:check` pass; `pnpm run format` has been run.
12. `crates/node-metrics` is added to the CI workflow's path filters, so its
    tests actually run when it changes. (Adding a test command to a filtered job
    is not enough if edits to the tested files don't trigger that job.)
13. No new binary is published: `local-build.sh` ships a fixed
    `build-<id>/bin/*` set, and the sampler is a library crate linked into the
    existing `vibe-kanban` and `vibe-kanban-worker` binaries.

## Testing

**Rust**

- `crates/node-metrics`: fixture-driven parser tests (`/proc/stat`, `meminfo`,
  `net_dev`, `loadavg`, `uptime`, `cpuinfo`, process `stat`/`status`/`cmdline`),
  including truncated, empty, and extra-column inputs; delta derivation across
  two fixtures including a counter that wraps or resets;
  first-sample-has-no-rates; ring-buffer retention and `since()` cursor/gap
  semantics.
- `crates/node-metrics/src/redact.rs`: a table of realistic command lines
  asserting every planted secret is masked and ordinary arguments survive.
- `crates/worker`: `GET /v1/metrics` rejects an unsigned request, rejects a
  signature computed over a different query string, and returns only samples
  after the cursor.
- `crates/services`: the collector isolates a failing node (one `Unreachable`,
  others `Available`); a metrics failure leaves `WorkerNode.status`, lease, and
  `eligibility()` untouched; the patch builder emits node-keyed operations that
  survive a worker being added and removed between ticks.

**Frontend**

- Vitest + testing-library in `packages/web-core`, following
  `WorkersSettingsSection.test.tsx`: the drawer renders one card per node;
  `Unreachable` / `Unsupported` / `NotImplemented` render their message rather
  than zeros; no socket is opened while closed; the sparkline path is generated
  from a known sample series.

**Deployment exercise** (the gate that local tests do not replace, per the
clustered-execution knowledge-base page)

- Open the drawer on `think2`, confirm three nodes report; run a CPU burner on
  `think3` and watch its meters move; stop `think4`'s worker and confirm
  `Unreachable` with scheduling unaffected; close the drawer and confirm the
  worker access log goes quiet.

## Risks

| Risk | Mitigation |
| --- | --- |
| Metrics failures leak into scheduling and strand work | No metrics path writes worker status, lease, or eligibility; asserted by test |
| Process command lines leak credentials | Redaction at the source before the sample is stored or transmitted; `environ` never read; fixture tests |
| Per-tick `/proc` process walk is expensive on a busy node | `spawn_blocking`, top-N cap, 2s cadence; collector idle when nobody is watching |
| A hung worker stalls the collector tick | Per-request 5s timeout, concurrent polls, one in-flight request per node |
| Array-index JSON patches corrupt the view on membership change | Node-keyed object patches; unit test over add/remove between ticks |
| Renaming scheduler-visible keys in `resource_snapshot` | `ResourceSnapshot` and the heartbeat are untouched; metrics use a separate channel |
| `worker_nodes.UNIQUE(hostname)` collision from a synthetic coordinator row | The coordinator node is synthesised at the API layer, never persisted |

## Resolved questions

All open questions were resolved in
[`specs/vk/3fd7-simple-server-me/clarifications.md`](specs/vk/3fd7-simple-server-me/clarifications.md):

1. **Redaction stays, non-configurable** (C1) — app access is not shell access;
   the UI is reachable through a public tunnel.
2. **Overlay, not a reflowing column** (C2) — it must open on every route, and a
   second docked column would fight the existing resizable-panel layout.
3. **Fixed 2s interval** (C3) — the cadence belongs to the node's sampler and is
   shared by all viewers, so it is not a per-viewer preference.
4. **5-minute window, process table not retained in history** (C4) — the table
   dominates the memory cost and nothing plots it over time.
5. **Stale readings stay visible, then expire at the retention bound** (C5) —
   "94% thirty seconds ago, then unreachable" is a better diagnosis than a blank
   panel; five-minute-old numbers are not.
6. The inconsistent worker-node React Query keys (`['worker-nodes']` vs
   `['workerNodes']`) remain **out of scope** — unrelated cleanup that would put
   the workspace-placement UI in this task's blast radius.
