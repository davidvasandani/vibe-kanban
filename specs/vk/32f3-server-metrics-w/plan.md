# Implementation Plan: Server Metrics Low-Disk Warnings

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical Context

Rust/Axum coordinator and remote APIs, Postgres remote issue storage via SQLx,
React/TypeScript/TanStack Query in `packages/web-core`, generated ts-rs types,
and NixOS service configuration in the scoped homelab module. The existing
metrics REST/WS path is `crates/server/src/routes/cluster_metrics`, backed by
`ClusterMetricsService`; the workspace accordion is assembled in
`RightSidebar.tsx` and rendered by `ServerMetricsSectionContainer.tsx`.

## Architecture & Approach

### 1. Effective thresholds in the existing snapshot

Add `DiskAlertThresholds` to `crates/node-metrics/src/types.rs`, with validated
environment loading in `crates/services/src/services/cluster/metrics.rs` at
coordinator startup and documented byte/percent defaults. The wire-type crate
does not read process-global configuration.
Carry effective thresholds on `ClusterMetricsSnapshot` in
`crates/services/src/services/cluster/metrics.rs`. JSON patches resnapshot this
stable field just as they do cadence; no second configuration request or disk
probe is introduced. Wire the four environment values in
`homelab/modules/vibe-kanban-rebuild.nix` as typed Nix options with assertions.

### 2. Pure alert derivation and accessible metrics UI

Create `diskAlerts.ts` beside the metrics views. Its pure functions validate
optional values, compute free percent from available/total, apply strict `<`
boundaries and the conservative OR rule, sort critical before warning, and
produce node/cluster rollups. `NodeStrip.tsx` receives derived alert data and
renders icon plus textual severity and `df -h`-style facts. `DisksPanel.tsx`
marks every affected filesystem and exposes total/used/available/use%.

The warning action is a separate focusable control rather than nesting a button
inside the node-selection button. Theme styling uses established semantic
tokens, text, borders, and an icon; it does not depend on color alone.

### 3. Collapsed header rollup without retaining the detail socket

Add `ServerMetricsHeader.tsx`, mounted through `RightSidebar`'s `headerExtra`.
It uses the existing host-scoped snapshot query key and REST API at a bounded
poll cadence, sharing cached data with the expanded fallback. It renders
nothing while evidence is absent and otherwise shows worst severity plus
affected-node count with bounded visible and full accessible/title text. The
existing detail body and WebSocket remain expansion-owned and unmounted while
collapsed.

### 4. Transactional remote resolve-or-create

Add API types for a structured low-disk observation/result. Add a dedicated
authenticated remote issue route and a local-server proxy through
`RemoteClient`. The coordinator proxy first resolves the node from a fresh
metrics snapshot, ignores client-supplied capacity facts, and forwards the
current server-owned affected set. The remote handler verifies project access, validates values,
and performs resolve-or-create in a single Postgres transaction. It takes a
transaction-scoped advisory lock derived from `(project_id, node_id,
"low_disk")`, queries `issues.extension_metadata` for the stable incident
identity joined to status, reuses a non-terminal match, otherwise creates the
issue using the project's first status and top sort order, and returns its
txid. The machine-readable metadata is additive and contains kind/node identity;
the human body is canonical Markdown with all affected filesystem facts and a
permanent-remediation checklist.

Using the remote issue database keeps deduplication durable across browser and
coordinator restarts. The advisory lock serializes concurrent requests where a
partial unique index cannot express “status name is not Done/Cancelled” across
the issue/status tables.

### 5. Frontend action and navigation

Pass the explicit `remoteProjectId` already available in `RightSidebar` into
the metrics container/header. Add an API client method for resolve-or-create.
The container keeps a module-scoped in-flight key for immediate duplicate-click
suppression, calls the server with the complete current affected set, waits for
the returned transaction through the existing Electric convergence helper, then navigates to the
returned issue. Without project context, the action is disabled with an
accessible explanation. Errors remain local to the action and do not hide
metrics.

## Data Model

See `./data-model.md`.

## Contracts

See `./contracts/low-disk-issue.md`.

## Research Notes

See `./research.md`. No new third-party dependency is planned.

## Constitution Check

- II: pure boundary tests, rendered-DOM tests, remote transaction/concurrency
  tests, and Nix evaluation cover the feature contract.
- III/VI: reuse the filesystem samples, snapshot/query identity, issue model,
  transaction response, and sidebar header seam.
- IV: data/state remains in web-core containers and presentation stays within
  the existing metrics views.
- V: remote creation and duplicate resolution use one txid-covered transaction.
- XIX: sampling stays read-only; only an explicit operator action creates an
  issue, and no scheduling/liveness state consumes alert severity.
- XXIII: the collapsed header preserves dynamic decision context independently
  of body lifecycle.

No deviation remains open.

## Risks & Dependencies

- Remote status has no explicit terminal flag; reuse the established
  case-insensitive Done/Cancelled convention required by Constitution V.
- Older coordinators/snapshots lack thresholds. Version-skew parsing must use a
  safe documented default without treating missing filesystem facts as zero.
- Header REST polling is intentionally bounded; tests must prove it stops when
  the surface unmounts and does not create an extra live socket.
- SQLx compile-time query metadata may require the repository's remote prepare
  workflow if a macro query changes.
- The checked-in SpecKit command assets still name another feature directory.
  This run treats those paths as stale template data and writes every artifact
  to the branch/task-scoped `specs/vk/32f3-server-metrics-w/` directory.
