# Tasks: Cluster Server Metrics

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes.

## Phase 1: Setup

- [ ] T001 Run `pnpm install --frozen-lockfile` at the repo root (required
      before any verification command; `pnpm run format` fails fast in
      `preformat` without it). No file changes.
- [ ] T002 Create the crate skeleton in `crates/node-metrics/Cargo.toml` and
      `crates/node-metrics/src/lib.rs`; add `"crates/node-metrics"` to
      `members` in `Cargo.toml`. Dependencies limited to existing workspace
      deps: `serde`, `serde_json`, `chrono`, `thiserror`, `tokio`, `tracing`,
      `ts-rs`, `uuid`. **No new third-party dependency** (research R1).
- [ ] T003 [P] Add `crates/node-metrics/**`, and the two pre-existing omissions
      `crates/worker/**` and `crates/cluster-protocol/**`, to the **`backend`**
      path filter in `.github/workflows/test.yml` (lines 58-69; it gates
      `backend-test` / `backend-clippy` / `backend-schema-checks`). Without this
      the new crate's tests never trigger CI (research R9). `packages/ui/**` is
      already in the `frontend` filter — no change needed there.
- [ ] T004 [P] Capture `/proc` fixtures into
      `crates/node-metrics/tests/fixtures/`: `stat`, `stat_next` (a second read
      for delta derivation), `meminfo`, `loadavg`, `uptime`, `cpuinfo`,
      `net_dev`, `net_dev_next`, `self_mounts`, and
      `pid/{stat,status,cmdline}`. Strip any real secret from the captured
      `cmdline` and add synthetic ones for the redaction tests.

## Phase 2: Sampling crate — pure surface

- [ ] T005 Define the wire types in `crates/node-metrics/src/types.rs`:
      `HostSample`, `CpuSample`, `MemorySample`, `FilesystemSample`,
      `NetworkSample`, `ProcessSample`, `SampleBatch`, `SamplerConfig`,
      `NodeRole`, `NodeMetricsAvailability`, `NodeHealth`. All derive
      `Debug, Clone, Serialize, Deserialize, TS`. Follow `../data-model.md`
      exactly on three points that are easy to get wrong and expensive to
      change later: **no float inside an internally-tagged enum** (invariant 1);
      **every rate-derived and possibly-unreadable field is `Option`** so
      "not readable" is never expressible as `0` (invariant 2 / FR-5 / FR-7);
      and `rename_all = "snake_case"` on every enum (invariant 6). `HostSample`
      carries `hostname` — it is the only source of one, since `ClusterConfig`
      has no such field. (depends on T002)
- [ ] T006 Add a `serde_json` round-trip test over `NodeMetricsAvailability` in
      `crates/node-metrics/src/types.rs` that fails if a float is ever added to
      a variant — the `preserve_order` hazard (`Cargo.toml:44`). (depends on
      T005)
- [ ] T007 [P] Implement the `/proc` parsers in
      `crates/node-metrics/src/parse.rs`: `parse_proc_stat`, `parse_meminfo`,
      `parse_loadavg`, `parse_uptime`, `parse_cpuinfo`, `parse_net_dev`,
      `parse_mounts`, `parse_process_stat`, `parse_process_status`,
      `parse_cmdline`. Each is a free function over `&str` returning
      `Option`/`Result` — never a fabricated zero. (depends on T005)
- [ ] T008 [P] Add per-parser unit tests in `crates/node-metrics/src/parse.rs`
      against the T004 fixtures plus truncated, empty, and extra-column inputs.
      (depends on T007)
- [ ] T009 [P] Implement the command-line redactor in
      `crates/node-metrics/src/redact.rs`: mask secret-keyed argument values
      (`--key=value` and `--key value`), bare credential-shaped tokens, URL
      userinfo, and known token prefixes; truncate to 256 chars. Mask
      longest-first so overlapping secrets cannot leak suffixes. (depends on
      T002)
- [ ] T010 [P] Add table-driven redaction tests in
      `crates/node-metrics/src/redact.rs` asserting that no planted secret
      appears anywhere in the output and that ordinary arguments survive.
      (depends on T009)
- [ ] T011 Implement rate derivation in `crates/node-metrics/src/derive.rs`:
      per-core CPU busy, per-process CPU, network bytes/sec, each computed
      against the previous **raw** counters. (depends on T007)
- [ ] T012 Add derivation tests in `crates/node-metrics/src/derive.rs`:
      two-fixture delta; **first-sample-has-no-rates — every rate-derived field
      is `None`, never `0`** (FR-7; a zero here would be a fabricated reading,
      which Constitution XIX prohibits outright); a counter that goes backwards
      yields `None` plus a `degraded` note, never a negative, a zero, or a
      wrapped spike; a long gap scales correctly because raw counters are
      stored, not derived values. (depends on T011)

## Phase 3: Sampling crate — host access

- [ ] T013 Implement the Linux collector in
      `crates/node-metrics/src/collect.rs`. The `/proc` walk distinguishes
      `ErrorKind::NotFound` (process exited — skip silently) from every other
      error (counted into `degraded`); use `try_exists()`, never `exists()`;
      never open `/proc/[pid]/environ`; apply T009's redactor before
      constructing a `ProcessSample`. Filesystem filter skips pseudo mounts but
      **keeps NFS**. Non-Linux builds compile to a stub returning
      `Unsupported { platform }`. (depends on T007, T009, T011)
- [ ] T014 Implement `MetricsSampler` in `crates/node-metrics/src/sampler.rs`:
      fixed-size ring sized by `retention`, monotonic `sequence`,
      `sample_now()`, `since(after) -> SampleBatch` carrying
      `earliest_retained_sequence`/`latest_sequence`, and `spawn()` whose loop
      runs the read on `spawn_blocking`, holds only a `Weak` to its owner, and
      exits when the consumer is gone. The ring stores samples **without** the
      process table; only the newest sample carries `processes` (clarification
      C4). (depends on T013)
- [ ] T015 Add sampler tests in `crates/node-metrics/src/sampler.rs`: ring
      retention evicts oldest; `since()` returns only newer samples; a cursor
      below `earliest_retained_sequence` reports the gap explicitly; retained
      history carries an empty process table while `latest` does not. (depends
      on T014)

## Phase 4: Worker

- [ ] T016 Add `node-metrics` to `crates/worker/Cargo.toml`; construct
      `Arc<MetricsSampler>` in `crates/worker/src/lib.rs::run()` and spawn its
      ticker with the existing shutdown token, passing the handle into
      `worker_api::router`. Do not touch the registration/heartbeat loop.
      (depends on T014)
- [ ] T017 Add `GET /v1/metrics?after={u64}` to the router in
      `crates/worker/src/worker_api.rs` (currently lines 89-114), **inside** the
      `require_signature` layer. No payload-level `RequestAuthority`, matching
      `GET /v1/jobs`. (depends on T016)
- [ ] T018 Add worker route tests in `crates/worker/src/worker_api.rs`: unsigned
      request → `401`; signature computed over a different `?after=` value →
      `401`; stale `x-vk-timestamp` → `401`; a valid request returns only
      samples after the cursor. (depends on T017)

## Phase 5: Coordinator services

- [ ] T019 Add `node-metrics` to `crates/services/Cargo.toml`, then add
      `WorkerClient::metrics(worker_node_id, after)` beside `inventory()` in
      `crates/services/src/services/cluster/client.rs` (currently lines
      279-290): fresh timestamp per call, signed target including `?after=`,
      per-request 5s timeout (not the client's 30s default at `:63`), explicit
      response body cap, and HTTP 404 mapped to a distinct error variant. There
      is no nonce on this transport for bodyless requests — do not add a header
      that the worker will not check (analysis E2). (depends on T017)
- [ ] T020 Implement `ClusterMetricsService` in
      `crates/services/src/services/cluster/metrics.rs`:
      - local coordinator sampler always running;
      - per-node rings keyed by `node_id`;
      - the synthetic coordinator node built from `ClusterConfig`, **never**
        persisted, with `node_id = coordinator_id` when set and
        `Uuid::new_v5(&COORDINATOR_NAMESPACE, hostname)` otherwise —
        `coordinator_id` is `Option` and `None` by default
        (`config.rs:41, :53`), and a `new_v4()` per boot would break the
        persisted node selection (analysis E7);
      - **every** worker row listed, including `Offline` ones (which are not
        polled but get `availability: NotCollected` and their real `health`) —
        omitting them freezes a lease-expired worker at `available`, the exact
        inversion FR-24 forbids (analysis E6);
      - `health` derived **read-only**: `lease_expires_at <= now` displays as
        `offline` without writing. **Do not call `expire_heartbeats`** — it
        issues `UPDATE worker_nodes SET status='offline'` and would make a
        lifecycle transition depend on a drawer being open (analysis E1);
      - subscriber-gated collector holding a `Weak`, re-checking the subscriber
        count each tick, never holding the node-map lock across an `await`,
        writing back generation-conditionally;
      - per-node failure isolation; retained readings expired past the retention
        window into `Stale`, then dropped (clarification C5).

      No path may write worker status, lease, or eligibility. (depends on T019,
      T014)
- [ ] T021 Export `ClusterMetricsService` from
      `crates/services/src/services/cluster/mod.rs` and construct it in
      `crates/local-deployment/src/lib.rs` beside the existing cluster services.
      (depends on T020)
- [ ] T022 Add services tests in
      `crates/services/src/services/cluster/metrics.rs`:
      - a failing node yields `Unreachable` while peers stay `Available`;
      - a 404 yields `NotImplemented`;
      - **a metrics failure leaves `WorkerNode.status`, `lease_expires_at`, and
        `eligibility()` byte-identical**;
      - **a *successful* snapshot build leaves `worker_nodes.status` and
        `updated_at` byte-identical** — the case that would have caught the
        `expire_heartbeats` write (analysis E1);
      - a lease-expired worker reports `health.status == offline` and
        `schedulable == false`, matching Settings, without a row write (FR-24);
      - `enabled: false` yields exactly one node, whose `node_id` is unchanged
        across a service restart (FR-2, FR-13);
      - the collector stops when subscribers reach zero;
      - a node deregistering mid-poll is not resurrected;
      - a node stale past the retention window has its readings dropped.
      (depends on T020)

## Phase 6: Coordinator HTTP surface

- [ ] T023 Add `node-metrics` to `crates/server/Cargo.toml`, then implement the
      patch builder as a pure function in
      `crates/server/src/routes/cluster_metrics.rs` (or a sibling module):
      resnapshot as `replace /nodes` + `replace /generated_at` — **not a root
      `replace`**, which the consuming hook cannot apply (analysis E4) — and
      per-tick node-keyed `replace /nodes/{id}/latest`,
      `add /nodes/{id}/history/-`, `remove /nodes/{id}/history/0`, emitted
      **once per sample received**, with nothing emitted for a node that
      returned no samples. Never array-index a node. (depends on T020)
- [ ] T024 Add patch-builder tests: adding and removing a worker between ticks
      emits correct operations and corrupts no other node; a replay gap or
      membership change forces a resnapshot; a zero-sample tick emits nothing
      for that node; a two-sample tick emits two appends and two evictions; a
      cold-start batch resnapshots instead of emitting N appends; the emitted
      payload does not grow with uptime; **the resnapshot round-trips through
      `applyUpsertPatch` semantics** (i.e. it targets an existing parent path).
      (depends on T023)
- [ ] T025 Add `GET /api/cluster/metrics` in
      `crates/server/src/routes/cluster_metrics.rs`, mounted from
      `admin_router()` in `crates/server/src/routes/workers.rs:33-37`. Triggers
      **one bounded collection round** if the collector is idle, then serves —
      otherwise, after the drawer has been shut longer than the retention
      window, it returns `latest: null` for every worker and is a fallback to
      nothing (analysis W2). It does not start the continuous collector.
      (depends on T021)
- [ ] T026 Add `GET /api/cluster/metrics/ws` in the same file using
      `SignedWsUpgrade` (`crates/server/src/middleware/signed_ws.rs:33`) and the
      `LogMsg::JsonPatch` idiom from `stream_approvals_ws`
      (`crates/server/src/routes/approvals.rs:51-106` — note that route does not
      use `MsgStore`). Increment the subscriber count on accept and decrement on
      close **including abnormal close**. Add a WebSocket ping on a fixed
      interval with a bounded pong deadline, dropping the connection and
      releasing its subscriber slot on timeout — close detection alone leaks a
      subscriber forever on a half-open connection (analysis W5). Emit a
      resnapshot every 30s. (depends on T023, T025)
- [ ] T027 Register the new `TS` decls in
      `crates/server/src/bin/generate_types.rs` (beside `WorkerNode` at lines
      75-77) and run `pnpm run generate-types`. Never hand-edit
      `shared/types.ts`. (depends on T005, T020)

## Phase 7: Frontend

- [ ] T028 [P] Add `clusterMetricsApi` to
      `packages/web-core/src/shared/lib/api.ts` using `makeHostAwareRequest`
      (`:197`), plus a key factory in
      `packages/web-core/src/shared/lib/clusterMetricsKeys.ts` that includes the
      host scope. (depends on T027)
- [ ] T029 [P] Add
      `packages/web-core/src/shared/stores/useMetricsDrawerStore.ts` — zustand +
      `persist`, key `metrics-drawer`, holding
      `{ open, width, selectedNodeId, expandedPanels }`. A dedicated store,
      modelled on `useOrgRailStore.ts:17`; **not** `useExpandableStore` (not
      persisted) and **not** the `useUiPreferencesStore` ↔ scratch round-trip.
      (depends on T001)
- [ ] T030 [P] Add `packages/ui/src/components/Sparkline.tsx` and
      `packages/ui/src/components/Meter.tsx` — stateless inline SVG and Tailwind
      width bars modelled on `ContextUsageGauge.tsx`, with `role="img"` and a
      value-stating `aria-label`, severity buckets on design tokens. **No
      charting library.** (depends on T001)
- [ ] T031 Add `packages/ui/src/components/MetricsDrawer.tsx` — right-anchored
      portal drawer mirroring `MobileDrawer.tsx:12` (`right-0`,
      `translate-x-full` closed, `transition-transform duration-200 ease-out`,
      `bg-black/50` backdrop). **Props-only**: `{ open, width, onWidthChange,
      onClose, children }`, with all state owned by the container —
      `@vibe/ui` has neither `zustand` nor `@vibe/web-core` in its
      `package.json`, so a store-aware drawer here would not typecheck
      (analysis W6). Renders 360–720px with drag-to-resize reported upward,
      `Escape` and backdrop close, focus trapped while open and restored on
      close. The scroll container carries **both** `overflow-y-auto` and
      `overflow-x-hidden`. (depends on T001)
- [ ] T032 [P] Add the panel views under
      `packages/web-core/src/shared/components/ui-new/views/metrics/`:
      `NodeStrip.tsx`, `CpuPanel.tsx`, `MemoryPanel.tsx`, `DisksPanel.tsx`,
      `NetworkPanel.tsx`, `ProcessesPanel.tsx`. These are feature views typed
      against generated metric types, not primitives, so Constitution IV puts
      them in `web-core` rather than `packages/ui` (analysis W6). Stateless,
      props-only, collapsible via the `CollapsibleSectionHeader` idiom. Process
      rows keyed by `(pid, start_ticks)`. `NodeStrip` renders **both**
      `health` and `availability` — they are different questions (FR-24). A
      `null` reading renders as "—", never as `0`. No no-op `<button>` —
      non-interactive rows are `<div>`s with an `aria-label`. (depends on T030,
      T027)
- [ ] T033 Add
      `packages/web-core/src/shared/components/ui-new/containers/ServerMetricsDrawerContainer.tsx`
      — one multiplexed `useJsonPatchWsStream` subscription open **only while
      the drawer is open**, with REST fallback; handles `error` and premature
      `close`; a `useRef` generation guard discards responses resolving after a
      host switch; each node panel wrapped in an error boundary; a transparent
      reconnect stays silent while a real error surfaces and is explicitly
      cleared on recovery with the debounce reset. (depends on T028, T031,
      T032)
- [ ] T034 Mount the drawer in
      `packages/web-core/src/shared/components/ui-new/containers/SharedAppLayout.tsx`
      beside `<MobileDrawer>` (~`:446`), behind a responsive wrapper so a mobile
      viewport never mounts the subscription. (depends on T033)
- [ ] T035 Add the `ToggleServerMetrics` action in
      `packages/web-core/src/shared/actions/index.ts` beside
      `ToggleRightSidebar` (`:621`) and add it to `NavbarActionGroups.right`
      (`:1561`). **Not** gated on `layoutMode === 'workspaces'`. Phosphor
      `PulseIcon`. (depends on T029)
- [ ] T036 [P] Add i18n keys with inline English defaults for every string in
      T031–T035, under `packages/web-core/src/i18n/locales/en/`. (depends on
      T035)

## Phase 8: Validation

- [ ] T037 [P] Add frontend tests beside `ServerMetricsDrawerContainer.tsx`:
      - one card per node;
      - `unreachable` / `unsupported` / `not_implemented` / `not_collected`
        render their message, and a `null` reading renders "—" — never `0`;
      - a `stale` node renders its retained readings de-emphasised with the
        capture timestamp (FR-18);
      - `health` and `availability` render independently, including the
        lease-expired-worker case (FR-24);
      - no socket is opened while the drawer is closed;
      - **drawer open state, width, selected node, and expanded sections
        survive a remount** (FR-13);
      - **keyboard: `Escape` dismisses, focus returns to the toggle, and every
        meter exposes its value as text** (FR-14 — Constitution II requires a
        rendered-DOM test for this surface);
      - **one node with malformed data does not blank the view** (FR-19);
      - the sparkline path is generated from a known series.

      Run via the package script so `NODE_ENV=test` is set (the dev shell
      exports `NODE_ENV=production`, which makes `act()` fail); use
      `vi.advanceTimersByTimeAsync(ms)`, never `runAllTimers`. `@vibe/ui`
      component tests run from the `remote-web` package. (depends on T033)
- [ ] T037a [P] Add a test asserting the redacted command never reaches a log
      sink: drive the collector with a fixture containing a planted secret and
      assert it appears in no `tracing` output (FR-26). (depends on T013)
- [ ] T038 Run repository verification in order: `cargo test --workspace`,
      `pnpm run check`, `pnpm run lint`, `pnpm run generate-types:check`,
      `pnpm run format`. (depends on T036, T037)
- [ ] T039 Runtime verification on this host: build the server, set
      `BACKEND_PORT` **and `PREVIEW_PROXY_PORT=0`** (worktrees inherit the host
      VK's port env vars), drive `/api/cluster/metrics/ws` with Node's built-in
      WebSocket, and confirm a root snapshot followed by node-keyed patches.
      (depends on T038)
- [ ] T040 Two-node deployment exercise — the gate local tests do not replace:
      - open the drawer on think2 and confirm three nodes report;
      - **compare each node's per-core CPU and memory against `btop` running on
        that same host** and confirm they agree within a few points;
      - **confirm the disks panel lists the shared NFS mount** with plausible
        used/total, and that writing a large file moves it;
      - run a CPU burner on think3 and watch only its meters move;
      - stop think4's worker and confirm `unreachable`, that its `health` still
        matches Settings, and that `worker_nodes.status`, lease, `updated_at`,
        and placement are unaffected;
      - close the drawer and confirm think4's access log goes quiet.
      (depends on T039)

## Phase 9: Documentation

- [ ] T041 [P] Add an operator note to `docs/` describing the drawer and what
      each panel reads. (depends on T034)
- [ ] T042 [P] Add a knowledge-base page under `docs/knowledge-base/` plus its
      `INDEX.md` row, tagged `3fd7-simple-server-me`. (depends on T040)

<!--
Conventions:
- `T001` … task ids are stable and referenced by the dependency graph.
- `[P]` … parallel-safe (independent files). Omit for tasks that must be serial.
- `[ ]` / `[x]` … completion checkbox, toggled from the workbench.
-->
