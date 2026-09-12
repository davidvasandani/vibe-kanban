# Implementation Plan: Cluster Server Metrics

**Spec**: `./spec.md`
**Clarifications**: `./clarifications.md`
**Status**: Draft

## Technical Context

Vibe Kanban is a Rust/Tokio/Axum backend with SQLite via SQLx and a
React/TypeScript frontend split across `packages/ui` (presentational),
`packages/web-core` (shared containers and data access), and the `local-web` /
`remote-web` entrypoints.

The cluster substrate this feature rides already exists, shipped by
`957e-clustered-vibe-k`:

- `crates/cluster-protocol` — transport-neutral coordinator↔worker messages.
- `crates/worker` — the `vibe-kanban-worker` binary; inbound signed `/v1/*`
  router in `crates/worker/src/worker_api.rs:89-114`, outbound registration and
  heartbeat loop in `crates/worker/src/server.rs:44-170`.
- `crates/services/src/services/cluster/` — `client.rs` (coordinator→worker
  reqwest transport, ed25519-signed), `registry.rs` (heartbeat sink, leases),
  `scheduler.rs`, `reconcile.rs`, `config.rs`.
- `crates/server/src/routes/workers.rs:33-48` — the admin and worker routers.
- `crates/db/src/models/worker_node.rs` — the `worker_nodes` row, including the
  untyped `resource_snapshot` JSON column.

Fleet: `think2` is the coordinator (`homelab/hosts/think/think2.nix:599-606`),
`think3` and `think4` are workers, all NixOS. The macmini is a client, not a
node. Everything is Linux, which is what makes a `/proc`-based collector viable.

Constraints that shape the design:

- `serde_json` is built with `preserve_order` workspace-wide
  (`Cargo.toml:44`), which breaks `f64` fields inside internally-tagged enums.
- `scheduler::score()` (`crates/services/src/services/cluster/scheduler.rs:80-90`)
  reads `load_1m` and `active_execution_count` out of `resource_snapshot` by
  string key. Renaming or nesting them makes every worker score `f64::INFINITY`.
- `worker_nodes` carries `UNIQUE(hostname)`
  (`crates/db/migrations/20260731000000_cluster_worker_persistence.sql:32`).
- There is no charting library in any `package.json`, and no `<canvas>` usage.
- `local-build.sh` publishes a fixed `build-<id>/bin/*` set; an unpublished
  binary is simply not deployed.

## Architecture & Approach

The shape is: **sample locally on every node → pull to the coordinator over the
existing signed channel → stream to the browser → render**. Four layers, each
testable before the one above it.

### Layer 1 — `crates/node-metrics` (new leaf crate)

Owns everything that reads a host and everything that derives a rate. Split so
that the tested surface is pure:

| Module | Responsibility |
| --- | --- |
| `types.rs` | `HostSample` and children, `SampleBatch`, `SamplerConfig`, `NodeMetricsAvailability`, `NodeRole`; all `TS`-derived |
| `parse.rs` | Free `&str → struct` parsers, one per `/proc` file |
| `derive.rs` | Rate derivation against the previous raw counters |
| `redact.rs` | Command-line masking (FR-26) |
| `collect.rs` | The only module that touches the filesystem; Linux-gated |
| `sampler.rs` | `MetricsSampler`: fixed ring, monotonic sequence, `since(after)`, spawnable ticker |

Maps FR-4/5/7 (what is measured, absent ≠ zero, rates need a predecessor),
FR-6/6a (bounded window, no process history), FR-25/26/27 (confidentiality).

The `/proc` walk distinguishes `ErrorKind::NotFound` (process exited — expected)
from any other error (counted into `degraded`), because
`read_dir(..).filter_map(|e| e.ok())` turns "couldn't read" into "not there" —
the trap recorded in `docs/knowledge-base/workspace-directory-reclamation.md`.
Presence checks use `try_exists()`, not `exists()`.

### Layer 2 — worker

`crates/worker/src/lib.rs::run()` constructs an `Arc<MetricsSampler>` and spawns
its ticker with the existing shutdown token. A new route
`GET /v1/metrics?after={u64}` is added to the router at
`crates/worker/src/worker_api.rs:89-114`, inside the layer that already applies
`require_signature` (`:409-463`) — so it inherits the ed25519 signature over
`{timestamp}.{METHOD}.{path_and_query}.{body_digest}` and the ±30s drift window.
Because the signed target includes the query string, a signature cannot be
reused against a different cursor (FR-28).

Like `GET /v1/jobs` (`:258`), it carries no payload-level `RequestAuthority` —
there is no body to bind one to. **This means no nonce check**: the nonce map is
consulted only in `validate_authority` (`:386-407`), which runs for
body-carrying routes. Verbatim replay inside the 30s drift window is therefore
possible, exactly as it already is for `/v1/jobs`, terminal output, and event
fetches. FR-28a records this as an accepted residual rather than a fixed
problem; see analysis E2. Earlier drafts of this plan claimed nonce coverage
here — that claim was false.

`ResourceSnapshot`, `WorkerHeartbeat`, and `PROTOCOL_VERSION` are untouched.
Metrics are a separate pull channel, which is what keeps FR-21 (no effect on
scheduling) structurally true rather than merely intended.

### Layer 3 — coordinator services

`crates/services/src/services/cluster/metrics.rs`, exported from the `cluster`
barrel (`mod.rs:7-13`) and constructed in `crates/local-deployment/src/lib.rs`
beside the existing cluster services.

- A local `MetricsSampler` for the coordinator, always running — one `/proc`
  read every 2s is negligible and it means the drawer has history the instant it
  opens (FR-2).
- Per-node rings keyed by `node_id`, plus cursor and `NodeMetricsAvailability`.
- `WorkerClient::metrics(worker_node_id, after)`, added beside `inventory()`
  (`client.rs:279-290`), which is the closest existing template — a signed GET
  with no body. It builds a **fresh timestamp per call**, uses a per-request 5s
  timeout rather than the client's 30s default (`client.rs:63`), and caps the
  response body before buffering.
- A collector task that runs **only while `subscriber_count > 0`** (FR-16),
  polls all non-`Offline` workers concurrently, holds only a `Weak`, re-checks
  the subscriber count every tick, and never holds the node map lock across an
  `await`. Write-back is generation-conditional so a worker that deregistered
  and re-registered during a poll is not resurrected.
- **Health is derived read-only.** FR-24 requires the drawer to agree with
  Settings about a lease-expired worker. It must *not* achieve that the way
  `list_workers` does — `WorkerRegistry::expire_heartbeats` issues
  `UPDATE worker_nodes SET status = 'offline'`
  (`crates/db/src/models/worker_node.rs:160-173`), so calling it here would make
  a real lifecycle transition fire because someone opened a monitoring panel.
  Instead the service reads `WorkerNode::fetch_all` and computes the displayed
  status in memory (`lease_expires_at <= now → offline`). See analysis E1.
- **Every worker row is listed, including `Offline` ones**, even though
  collection skips them. Omitting them would freeze a lease-expired worker's
  `availability` at its last value — plausibly `available` — which is the exact
  inversion FR-24 forbids. A skipped node gets `availability: NotCollected` plus
  its real `health`.
- Per-node failure isolation (FR-19). No path writes worker status, lease, or
  eligibility (FR-21, FR-22).
- The coordinator pseudo-node is synthesised here from `ClusterConfig`, never
  persisted (FR-3), which sidesteps `UNIQUE(hostname)`.
- Retained readings for an unreachable node expire once the newest sample is
  older than the retention window (FR-18, C5).

### Layer 4 — coordinator HTTP surface

New `crates/server/src/routes/cluster_metrics.rs`, mounted from the existing
`admin_router()`:

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/api/cluster/metrics` | Snapshot; serves current rings without starting the collector |
| GET | `/api/cluster/metrics/ws` | Snapshot-then-patch stream |

The WS handler uses `SignedWsUpgrade`
(`crates/server/src/middleware/signed_ws.rs:33`) and drives a
`LogMsg::JsonPatch` stream straight onto the socket, following
`stream_approvals_ws` (`crates/server/src/routes/approvals.rs:51-106`). Note
that route does **not** use `MsgStore` — it drives
`deployment.approvals().patch_stream()` directly; an earlier draft of this plan
cited `MsgStore` in error. The client is the existing `useJsonPatchWsStream`
hook with no new frontend plumbing.

The patch builder is a separate, pure function so it can be unit-tested without
a socket. It emits node-keyed operations (`/nodes/{node_id}/…`) — never array
indices, because a worker registering mid-stream would otherwise shift every
index and land a `replace` on the wrong node (FR-11, and the identity trap in
`docs/knowledge-base/claude-log-normalization.md`).

The resnapshot — emitted on connect, every 30s, and on any replay gap or
membership change — targets **`/nodes` and `/generated_at`, not the document
root**. A `replace` at path `""` is unapplicable by the consuming hook: it
mutates an Immer draft, and an empty rfc6902 pointer yields `parent === null`,
so the op fails and the `add` retry attempts `null[''] = value`. The backstop
would silently never land. See analysis E4.

A tick carries however many samples the poll returned — 0, 1, 2, or (on the
first poll after connect) the whole ring. The builder emits one append and one
eviction **per sample**, emits nothing for a node that returned nothing, and
resnapshots rather than emitting a hundred appends for a cold-start batch.

New types are registered in `crates/server/src/bin/generate_types.rs:75-77`
beside `WorkerNode`, then `pnpm run generate-types`.

### Layer 5 — frontend

| Concern | File |
| --- | --- |
| API + host-scoped query key | `packages/web-core/src/shared/lib/api.ts` (`clusterMetricsApi`, via `makeHostAwareRequest:197`) |
| Persisted drawer state | `packages/web-core/src/shared/stores/useMetricsDrawerStore.ts` (zustand + `persist`, modelled on `useOrgRailStore.ts:17`) |
| Drawer shell (**props-only**) | `packages/ui/src/components/MetricsDrawer.tsx`, mirroring `MobileDrawer.tsx:12` with `right-0` / `translate-x-full` |
| Meters and sparklines (primitives) | `packages/ui/src/components/{Meter,Sparkline}.tsx`, modelled on `ContextUsageGauge.tsx` |
| Panels (feature views) | `packages/web-core/src/shared/components/ui-new/views/metrics/{NodeStrip,CpuPanel,MemoryPanel,DisksPanel,NetworkPanel,ProcessesPanel}.tsx` |
| Container | `packages/web-core/src/shared/components/ui-new/containers/ServerMetricsDrawerContainer.tsx` |
| Mount point | `SharedAppLayout.tsx` beside `<MobileDrawer>` (~`:446`) |
| Toggle | `ToggleServerMetrics` in `packages/web-core/src/shared/actions/index.ts` (beside `ToggleRightSidebar:621`), added to `NavbarActionGroups.right:1561` |

Overlay, not a reflowing column (FR-9a / C2). One multiplexed socket for all
nodes, open only while the drawer is open. Host-scoped query keys with a
`useRef` generation guard so a late response for a deselected host is dropped.
Each node panel wrapped in an error boundary (FR-19). The scroll container
carries `overflow-x-hidden` alongside `overflow-y-auto`. Hooks are gated behind
a responsive wrapper so a mobile viewport never mounts the subscription.
Hand-rolled SVG — **no charting library is added**.

## Data Model

See [`./data-model.md`](./data-model.md).

## Contracts

See [`./contracts/worker-metrics.md`](./contracts/worker-metrics.md) and
[`./contracts/coordinator-metrics-api.md`](./contracts/coordinator-metrics-api.md).

## Research Notes

See [`./research.md`](./research.md) — in particular the decision **not** to add
`sysinfo` or any other new dependency, and the decision not to add a sixth
binary.

## Constitution Check

Checked against `.specify/memory/constitution.md` v0.17.0.

| Principle | How this plan honors it |
| --- | --- |
| I. Clarity over cleverness | Parsers are plain functions over `&str`; the collector mirrors the existing hand-rolled `/proc` reads in `worker/src/server.rs:320` |
| II. Test the contract | Every layer names its tests before it is built; acceptance criteria are concrete and checkable |
| III. Small, reversible steps | Reuses `useJsonPatchWsStream`, `LogMsg::JsonPatch`, `SignedWsUpgrade`, `MobileDrawer`, `ContextUsageGauge`; the drawer is a portal sibling with no layout coupling, so removing it removes nothing else |
| IV. Shared-component boundaries | `packages/ui` gets only genuine primitives (`Meter`, `Sparkline`) plus a props-only `MetricsDrawer` shell. The feature panels are typed against generated metric types and are **not** primitives, so they live in `web-core` alongside the container. `@vibe/ui` also has neither `zustand` nor `@vibe/web-core` in its `package.json`, so a store-aware drawer there would not even typecheck. See analysis W6 |
| V. Remote mutations transactional | N/A — this feature performs no mutation |
| VI. Don't rebuild what shipped | Extends the existing signed coordinator↔worker channel, registry, and route tree rather than adding a parallel one |
| XI. Diagnostics are evidence | `degraded` notes and availability reasons are surfaced verbatim, not summarised away |
| XII. Asynchronous handoffs | The collector's lifetime is owned by the subscriber count with a `Weak` handle and a per-tick liveness check |
| XVII. Live capability state | Availability is confirmed per poll, not inferred; a stale reading is labelled with its own timestamp |
| XVIII. Distributed execution | **The critical one.** No metrics path writes status, lease, or eligibility — including the health derivation, which is computed in memory rather than via `expire_heartbeats`. A metrics timeout is not evidence. The signed target includes path and query, so a signature cannot be reused against a different cursor. Verbatim replay within the drift window is an accepted residual (FR-28a) |
| XIX. Observability read-only | The principle this feature motivated: typed absence, bounded self-correcting stream, node-keyed patches, terminating sampler, redaction at source, `environ` never read |

**Deviations: two, both recorded as accepted residuals in `analysis.md`.**

1. **FR-28a / E2** — `GET /v1/metrics` is replayable verbatim within the 30s
   timestamp-drift window, because the shared transport carries no nonce for
   bodyless requests. Every other read-only cluster endpoint has the same
   property. Closing it means touching transport code used by every `/v1/*`
   route to protect a fetch that yields nothing a valid signature holder could
   not already obtain. Reopens if a *mutating* bodyless route is ever added.
2. **FR-20a / W4** — post-connection stream interruptions surface as per-node
   staleness rather than a view-level error, because `useJsonPatchWsStream`
   owns that state and never reports an error once data has arrived. Fixing it
   properly means changing a hook shared by five features.

One pre-existing violation is *surfaced* rather than introduced: the CI
`backend` path filter (`.github/workflows/test.yml:58-69` — the filter output is
named `backend`, feeding `backend-test`/`backend-clippy`/`backend-schema-checks`)
enumerates crates explicitly and omits `crates/worker` and
`crates/cluster-protocol`, so those crates' tests do not trigger CI when only
they change. This plan adds `crates/node-metrics` **and** the two missing
entries, per the rule in
`docs/knowledge-base/worktree-formatting-prerequisites.md` that a test command
in a filtered job is useless if edits to the tested files don't trigger it.
`packages/ui/**` is already covered by the `frontend` filter.

## Risks & Dependencies

| Risk | Mitigation |
| --- | --- |
| A metrics failure leaks into scheduling and strands work | No metrics path writes worker status, lease, or eligibility; asserted by a test that snapshots those fields across a failed poll |
| A process command line leaks a credential | Redaction inside the sampler before storage or transmission; `environ` never opened; table-driven tests assert no planted secret survives |
| The `/proc` process walk is expensive on a busy node | `spawn_blocking`, top-N cap, 2s cadence, collector idle with no subscribers |
| A hung worker stalls the collector tick | Per-request 5s timeout, concurrent polls, one in-flight request per node |
| Array-index patches corrupt the view on membership change | Node-keyed object patches plus a unit test over add/remove between ticks |
| A float inside a tagged enum fails to deserialize under `preserve_order` | All percentages live in plain structs; a round-trip test over `NodeMetricsAvailability` guards the invariant |
| Retained stale readings are misread as current | Timestamped and de-emphasised, then dropped past the retention bound (C5) |
| A new crate's tests silently never run in CI | Path filter updated in the same change |

**Depends on:** the cluster substrate from `957e-clustered-vibe-k` being
deployed on think3/think4. A worker running an older build degrades to
`NotImplemented` rather than breaking, so the two can deploy independently.
