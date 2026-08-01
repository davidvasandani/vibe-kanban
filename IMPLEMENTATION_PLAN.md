# Implementation Plan: Cluster Server Metrics (`3fd7-simple-server-me`)

Companion to [`SPEC.md`](SPEC.md) and [`PRIOR_KNOWLEDGE.md`](PRIOR_KNOWLEDGE.md).
Steps are ordered so every layer is testable before the one above it exists.
`[P]` marks work that can proceed in parallel within its layer.

## Layer 0 — Setup

1. `pnpm install --frozen-lockfile` in this worktree (required before any
   verification command; `pnpm run format` fails fast in `preformat` without it).
2. Capture `/proc` fixtures from a real NixOS host into
   `crates/node-metrics/tests/fixtures/`: `stat`, `meminfo`, `loadavg`,
   `uptime`, `cpuinfo`, `net_dev`, `self_mounts`, and a `pid/{stat,status,cmdline}`
   triple. Capture two consecutive `stat`/`net_dev` reads so delta derivation is
   testable. Redact any real secrets out of the captured `cmdline` and add
   synthetic ones for the redaction tests.

## Layer 1 — `crates/node-metrics` (pure, no I/O in the tested surface)

3. Create the crate: `Cargo.toml` (deps `serde`, `chrono`, `thiserror`, `ts-rs`
   — all existing workspace deps, **no new third-party crate**), add it to
   `members` in the root `Cargo.toml`, and add its path to the CI workflow's
   change filters so its tests actually run.
4. `types.rs` — `HostSample`, `CpuSample`, `MemorySample`, `FilesystemSample`,
   `NetworkSample`, `ProcessSample`, `SampleBatch`, `SamplerConfig`,
   `NodeMetricsAvailability`, `NodeRole`. All derive
   `Debug, Clone, Serialize, Deserialize, TS`. **Assert by construction that no
   float sits inside an internally-tagged enum variant** (the `preserve_order`
   trap); add a round-trip `serde_json` test over `NodeMetricsAvailability` that
   would catch it if someone later adds one.
5. `[P]` `parse.rs` — free `&str → struct` parsers: `parse_proc_stat`,
   `parse_meminfo`, `parse_loadavg`, `parse_uptime`, `parse_cpuinfo`,
   `parse_net_dev`, `parse_mounts`, `parse_process_stat`, `parse_process_status`,
   `parse_cmdline`. Each returns `Option`/`Result`, never a fabricated zero.
   Unit tests per parser over the fixtures plus truncated, empty, and
   extra-column inputs.
6. `[P]` `redact.rs` — the command-line redactor from `SPEC.md` §6. Table-driven
   tests over realistic command lines; each case asserts the planted secret does
   not appear anywhere in the output and that ordinary arguments survive.
   Redaction is applied **inside** the sampler, before a sample is stored.
7. `derive.rs` — rate derivation against a previous raw read: per-core CPU busy,
   per-process CPU, network bytes/sec. Tests: two-fixture delta;
   first-sample-has-no-rates (all rates `0`, `degraded` note present); a counter
   that goes backwards yields `0` plus a `degraded` note, not a negative or a
   huge spike; a long gap produces a correctly-scaled rate (raw counters stored,
   not derived values).
8. `collect.rs` — the Linux collector that reads the real files and calls the
   parsers. The `/proc` walk distinguishes `ErrorKind::NotFound` (process
   exited — skip) from any other error (counted into `degraded`), and uses
   `try_exists()` rather than `exists()`. Non-Linux builds compile to a stub
   returning `Unsupported { platform }`.
9. `sampler.rs` — `MetricsSampler`: fixed-size ring (`retention`), monotonic
   `sequence`, `sample_now()`, `since(after) -> SampleBatch` with
   `earliest_retained_sequence` / `latest_sequence`, and `spawn()` whose loop
   runs the read on `spawn_blocking`, holds only a `Weak` to its owner, and
   exits when the consumer is gone. Tests: ring retention evicts oldest;
   `since()` returns only newer samples; a cursor older than
   `earliest_retained_sequence` reports the gap explicitly.

## Layer 2 — Worker

10. `crates/worker/src/lib.rs::run()` — construct `Arc<MetricsSampler>`, spawn
    its ticker with the existing shutdown token, and pass the handle into
    `worker_api::router`. Do not touch the registration/heartbeat loop,
    `ResourceSnapshot`, `WorkerHeartbeat`, or `PROTOCOL_VERSION`.
11. `crates/worker/src/worker_api.rs` — add
    `GET /v1/metrics?after={u64}` → `SampleBatch` inside the router that already
    carries `require_signature`. No payload-level `RequestAuthority` (no body),
    matching `GET /v1/jobs`.
12. Worker tests: unsigned request → `401`; a signature computed over a
    different `?after=` value → `401`; a stale timestamp → `401`; a valid
    request returns only samples after the cursor.

## Layer 3 — Coordinator services

13. `crates/services/src/services/cluster/client.rs` — add
    `metrics(worker_node_id, after) -> Result<SampleBatch, WorkerClientError>`
    beside `inventory()`. It builds a **fresh timestamp and nonce per call**,
    signs the full target including `?after=`, uses a **per-request 5s timeout**
    (not the client's 30s default), and caps the response body before buffering.
    Map HTTP 404 to a distinct error so the caller can report `NotImplemented`
    rather than `Unreachable`.
14. `crates/services/src/services/cluster/metrics.rs` — `ClusterMetricsService`:
    - a local `MetricsSampler` for the coordinator, always running;
    - per-node rings, cursors, and `NodeMetricsAvailability`, keyed by
      `node_id`;
    - a subscriber counter; the worker collector runs **only** while it is
      `> 0`, re-checks it every tick, holds a `Weak`, never holds the node-map
      lock across an `await`, and writes back generation-conditionally;
    - `expire_heartbeats(now)` before every node listing;
    - per-node failure isolation — one node's error never affects another's
      status, and **no path** writes worker status, lease, or eligibility;
    - the coordinator pseudo-node synthesised from `cluster_config`, never
      persisted to `worker_nodes`.
15. Export it from `crates/services/src/services/cluster/mod.rs` and construct
    it in `crates/local-deployment/src/lib.rs` beside the existing cluster
    services.
16. Services tests: a failing node yields `Unreachable` while its peers stay
    `Available`; a 404 yields `NotImplemented`; a metrics failure leaves
    `WorkerNode.status`, `lease_expires_at`, and `eligibility()` byte-identical;
    the collector stops when subscribers reach zero; a node deregistering
    mid-poll does not resurrect its entry.

## Layer 4 — Coordinator HTTP surface

17. Patch builder (unit-testable, separate from the route): produce a root
    `replace` snapshot, and per-tick node-keyed `replace`/`add`/`remove`
    operations. Tests: adding and removing a worker between ticks produces
    correct operations; a replay gap or node-set change forces a full
    resnapshot; the emitted patch payload does not grow with uptime.
18. Routes in `crates/server/src/routes/` (new `cluster_metrics.rs`, mounted
    from the existing `admin_router()`):
    - `GET /api/cluster/metrics` → `ApiResponse<ClusterMetricsSnapshot>`, serves
      the current rings without starting the collector;
    - `GET /api/cluster/metrics/ws` → `SignedWsUpgrade` + `MsgStore` /
      `LogMsg::JsonPatch`, following `/api/approvals/stream/ws`. Increment the
      subscriber count on accept, decrement on close **including abnormal
      close**. Emit a full resnapshot every 30s as the convergence backstop.
19. `crates/server/src/bin/generate_types.rs` — register the new `TS` decls
    beside `WorkerNode`/`WorkerNodeStatus`; run `pnpm run generate-types`. Never
    hand-edit `shared/types.ts`.

## Layer 5 — Frontend

20. `[P]` `packages/web-core/src/shared/lib/api.ts` — `clusterMetricsApi.get()`
    via `makeHostAwareRequest`, plus the host-scoped query key. Add a
    `clusterMetricsKeys.ts` factory alongside the existing key factories.
21. `[P]` `packages/web-core/src/shared/stores/useMetricsDrawerStore.ts` —
    zustand + `persist`, key `metrics-drawer`, holding
    `{ open, width, selectedNodeId, expandedPanels }`. A dedicated store, not
    `useExpandableStore` (which is deliberately not persisted) and not the
    `useUiPreferencesStore` ↔ scratch ↔ Rust round-trip.
22. `[P]` `packages/ui/src/components/Sparkline.tsx` and `Meter.tsx` — stateless
    inline SVG / Tailwind bars modelled on `ContextUsageGauge`, with
    `role="img"` and a value-stating `aria-label`. Severity buckets use the
    `ContextUsageGauge` token convention. **No charting library is added.**
23. `packages/ui/src/components/MetricsDrawer.tsx` — right-anchored portal
    drawer mirroring `MobileDrawer` (`right-0`, `translate-x-full` closed,
    `transition-transform duration-200 ease-out`, `bg-black/50` backdrop).
    Default 420px, drag-resizable 360–720px. `Escape` and backdrop close; focus
    trapped while open and restored to the toggle on close. The scroll container
    carries **both** `overflow-y-auto` and `overflow-x-hidden`.
24. `[P]` Panel views in `packages/ui`: `CpuPanel`, `MemoryPanel`, `DisksPanel`,
    `NetworkPanel`, `ProcessesPanel`, `NodeStrip`. Stateless, props-only,
    collapsible via the `CollapsibleSectionHeader` idiom. Process rows are
    keyed by `(pid, start_ticks)`. No interactive control that does nothing —
    non-interactive rows are `<div>`s with an `aria-label`, not no-op `<button>`s.
25. `ServerMetricsDrawerContainer.tsx` in web-core — one multiplexed
    `useJsonPatchWsStream` subscription, **open only while the drawer is open**,
    with REST fallback. Handles `error` and premature `close`. A `useRef`
    generation guard discards responses that resolve after the host changed.
    Each node panel is wrapped in an error boundary. A transparent reconnect is
    silent; a real error surfaces and is explicitly cleared on recovery, with
    the error-report debounce reset.
26. Mount it in `SharedAppLayout.tsx` beside `<MobileDrawer>`, behind a
    responsive wrapper so a mobile viewport never mounts the subscription.
27. Toggle: `ToggleServerMetrics` in `packages/web-core/src/shared/actions/index.ts`
    beside `ToggleRightSidebar`, added to `NavbarActionGroups.right`, **not**
    gated on `layoutMode === 'workspaces'`. Phosphor `PulseIcon`.
28. i18n: keys with inline English defaults; `pnpm local-web:lint:i18n` clean.

## Layer 6 — Tests and verification

29. Frontend tests (vitest + testing-library, run via the package script so
    `NODE_ENV=test` is set — the dev shell exports `NODE_ENV=production`, which
    makes `act()` fail): the drawer renders one card per node;
    `Unreachable` / `Unsupported` / `NotImplemented` render their message rather
    than zeros; no socket is opened while the drawer is closed; the sparkline
    path is generated from a known series. Interval-driven cases use
    `vi.advanceTimersByTimeAsync(ms)`, never `runAllTimers`. `@vibe/ui`
    component tests run from the `remote-web` package.
30. Repository verification, in order: `cargo test --workspace`,
    `pnpm run check`, `pnpm run lint`, `pnpm run generate-types:check`,
    `pnpm run format`.
31. Runtime verification on this host (the headless recipe from the
    browser-session knowledge-base page): build the server, set `BACKEND_PORT`
    **and `PREVIEW_PROXY_PORT=0`** (worktrees inherit the host VK's port env
    vars), drive `/api/cluster/metrics/ws` with Node's built-in WebSocket, and
    confirm a snapshot followed by node-keyed patches.
32. Two-node deployment exercise — the gate local tests do not replace: open the
    drawer on think2 and confirm three nodes report; run a CPU burner on think3
    and watch its meters move; stop think4's worker and confirm `Unreachable`
    with `worker_nodes.status`, lease, and placement unaffected; close the
    drawer and confirm think4's access log goes quiet.

## Layer 7 — Documentation

33. Update `docs/` with a short operator note on the drawer and what each panel
    reads, and record the reusable knowledge in `docs/knowledge-base/`
    (new page + `INDEX.md` row, tagged `3fd7-simple-server-me`).

## Sequencing notes

- Layers 1→2→3→4 are strictly ordered: the worker route needs the sampler, the
  client needs the route, the service needs the client, the routes need the
  service.
- Layer 5 can start against a hand-written fixture of `ClusterMetricsSnapshot`
  as soon as step 4 fixes the types, in parallel with Layers 2–4.
- Nothing in this plan changes `crates/cluster-protocol`, the heartbeat, the
  scheduler, the `worker_nodes` schema, or the homelab Nix module. If any step
  starts to require one of those, stop — that is a spec change, not an
  implementation detail.
