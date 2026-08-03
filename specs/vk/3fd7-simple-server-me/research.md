# Research Notes: Cluster Server Metrics (`3fd7-simple-server-me`)

Decisions, alternatives considered, and rationale. Per the constitution's
constraints, every candidate new dependency is recorded here with its outcome.

---

## R1. Collect via hand-rolled `/proc` parsing, not `sysinfo`

**Decision: hand-rolled. No new dependency.**

Candidates considered: `sysinfo` (the obvious default), `procfs`, `heim`.

`sysinfo` would give cross-platform collection and a maintained process walker
for roughly zero code. Against that:

1. **The fleet is Linux-only.** `think2`/`think3`/`think4` are NixOS; the one
   macOS machine in `think-cluster` is an SSH client, not a node
   (`homelab/hosts/macmini-m2.nix`). Cross-platform support is the main thing
   `sysinfo` sells and we would not use it.
2. **The workspace already does this.** `resource_snapshot()` at
   `crates/worker/src/server.rs:320-347` hand-parses `/proc/loadavg` and
   `/proc/meminfo` today. Adding a dependency that duplicates an existing local
   idiom, without replacing it, leaves two ways to read the same files.
3. **Testability, which is the deciding factor.** The risky logic here is not
   "read the file", it is *delta derivation* (per-core CPU, per-process CPU,
   network rates) and *redaction*. Pure `&str → struct` parsers let both be
   tested exhaustively against checked-in fixtures — including truncated files,
   counter resets, and planted secrets — with no host access and no mocking.
   `sysinfo` returns populated structs from a live host; you cannot hand it a
   fixture, so the interesting cases become untestable in CI.
4. **Precedent.** `docs/knowledge-base/mcp-connectivity-testing.md` records the
   same call being made for the MCP probe: a minimal hand-rolled client on
   existing crates, chosen after checking whether an existing crate could do it.

Cost accepted: roughly 400 lines of parsing and a fixture set, plus ownership of
`/proc` format quirks. The formats in question are stable kernel ABI.

**Reopens if:** a non-Linux node ever needs to report, at which point `sysinfo`
behind the same `collect.rs` seam is the migration, and the parser tests remain
valid for the Linux path.

---

## R2. Pull per-node, don't extend the heartbeat

**Decision: a new `GET /v1/metrics` pull channel. The heartbeat is untouched.**

The heartbeat already carries a `ResourceSnapshot`
(`crates/cluster-protocol/src/lib.rs:46-52`) and extending it looks cheaper.
Rejected for three reasons:

1. **Cadence mismatch.** The heartbeat runs at the coordinator-dictated
   `heartbeat_interval_seconds`, default 10s
   (`crates/services/src/services/cluster/config.rs:19`). btop-class output needs
   ~2s. Speeding the heartbeat up to 2s would quintuple the write rate to
   `worker_nodes` and shorten every lease calculation that depends on it.
2. **Blast radius.** `scheduler::score()` reads `load_1m` and
   `active_execution_count` out of `resource_snapshot` by string key
   (`scheduler.rs:80-90`), falling back to `f64::INFINITY`. Anything that
   reshapes that blob risks silently making every worker unschedulable. A
   separate channel makes "metrics cannot affect scheduling" a structural fact
   rather than a promise.
3. **Idle cost.** Push means every worker reports metrics forever whether or not
   anyone is looking. Pull lets the coordinator collect only while a subscriber
   exists (FR-16), so an idle cluster costs one local `/proc` read.

The pull direction also reuses `WorkerClient`'s existing signing, and
`inventory()` (`client.rs:279-290`) is a signed GET with no body — an exact
template.

---

## R3. Coordinator represented as a synthetic node, not a `worker_nodes` row

**Decision: synthesise at the service layer; never persist.**

Alternatives: (a) insert a real row for the coordinator; (b) a separate
`/api/coordinator/metrics` endpoint; (c) synthesise.

(a) collides with `UNIQUE(hostname)`
(`crates/db/migrations/20260731000000_cluster_worker_persistence.sql:32`) if the
coordinator host ever also runs a worker, and — worse — a row in `worker_nodes`
is by definition visible to `scheduler::eligibility()` and
`WorkerNode::fetch_all`. A synthetic row would mean the metrics feature had
inserted something the scheduler can see, which is precisely what FR-21 and
Constitution XVIII forbid.

(b) forces the frontend to merge two differently-shaped sources and special-case
the coordinator in every panel, which
`wiki/managed-cli-tool-catalog.md`'s "keep it generic, express the difference as
data" rule argues against.

(c) gives one uniform `MetricsNode` list with `role` as a data field. It also
makes the single-machine case (clustering disabled, zero worker rows) work with
no extra code path — the list simply has one entry.

---

## R4. Server-side ring buffer, not client-side history accumulation

**Decision: retain history on the coordinator.**

Client-side accumulation is simpler and needs no server state. Rejected because:

- Opening the drawer would show empty graphs that fill in over five minutes.
  The most common reason to open it is *something is wrong now*, and a blank
  graph is exactly the wrong answer.
- Every client would re-derive the same history, and a second browser tab would
  disagree with the first.
- `docs/knowledge-base/collapsing-repeated-log-entries.md` records the
  established rule: do this kind of work server-side so every consumer benefits.

The cost is bounded memory on the coordinator, sized in R5.

---

## R5. Window size, and why the process table is excluded from history

**Decision: 150 samples ≈ 5 minutes; history excludes the process table.**

Rough sizing per sample on an 8-core node with ~6 filesystems and ~3 interfaces:

| Part | Approx. size |
| --- | --- |
| CPU (8 cores + scalars) | ~120 B |
| Memory | ~60 B |
| Filesystems (6) | ~400 B |
| Networks (3) | ~200 B |
| **Processes (15)** | **~3,000 B** |

The process table is ~80% of a sample and *nothing plots it over time*. Keeping
it in the ring costs ~450 KB per node (~1.4 MB across three nodes) to store data
no panel reads. Excluding it drops per-node retention to ~150 KB and keeps every
streamed patch small — which is the actual point of Constitution XIX's bounded
stream rule, and of the "never emit a payload that grows with uptime" lesson in
`docs/knowledge-base/collapsing-repeated-log-entries.md`.

Five minutes is chosen because it answers the question the graphs exist to
answer ("spike or sustained?") and fills a sparkline at the drawer's default
420px width at a legible density.

---

## R6. Transport: reuse the JSON-Patch WebSocket, don't invent one

**Decision: `SignedWsUpgrade` + `LogMsg::JsonPatch` driven straight onto the
socket, consumed by the existing `useJsonPatchWsStream`.** (The approvals route
this follows does not use `MsgStore`; it drives
`deployment.approvals().patch_stream()` directly.)

Alternatives: React Query polling at 2s (what `WorkersSettingsSection.tsx:19-23`
does at 10s); Server-Sent Events; a bespoke WS message format.

Polling was the tempting minimal option, and it is the fallback path. As the
primary it is wrong: each poll would ship the entire five-minute window for
every node — the payload is dominated by history that has not changed — where a
patch ships only the newest sample. At 2s across three nodes that is the
difference between roughly 500 KB/s and a few KB/s.

SSE exists in the codebase (`crates/server/src/routes/events.rs`) but is used
for exactly one endpoint; WebSockets are the dominant streaming idiom here
(approvals, raw logs, normalized logs, scratch, browser sessions). Following the
majority idiom means `useJsonPatchWsStream` works unmodified.

A bespoke format was rejected on the "don't invent a second dialect" grounds in
`docs/knowledge-base/claude-log-normalization.md`.

**Gotcha this decision inherits:** `serde_json` is compiled with
`preserve_order` (`Cargo.toml:44`), under which an internally-tagged enum with
an `f32`/`f64` field fails to deserialize (`invalid type: map, expected f64`) —
documented in `wiki/browser-session-control-arbiter.md`. All percentages and
load averages therefore live in plain structs, never in a tagged variant, and a
round-trip test guards it.

---

## R7. No charting library

**Decision: hand-rolled inline SVG and Tailwind width bars.**

Verified absent from every `package.json` and from `pnpm-lock.yaml`: `recharts`,
`visx`, `uplot`, `chart.js`, `echarts`, `apexcharts`, `victory`, `nivo`. `d3-*`
appears only as a transitive dependency of `mermaid`, and pnpm's strict
`node_modules` means `import 'd3'` will not resolve.

What this feature actually draws is bars and polylines. `ContextUsageGauge.tsx`
already establishes the idiom — hand-computed SVG geometry, threshold buckets
mapped to design tokens, `role="img"` with a value-stating `aria-label`. Adding
a charting library would be the only new frontend dependency in the change, for
marks that are a dozen lines of SVG.

---

## R8. No new binary

**Decision: `node-metrics` is a library crate linked into the existing
`vibe-kanban` and `vibe-kanban-worker` binaries.**

`wiki/self-hosted-deployment.md` records that `local-build.sh` publishes a fixed
set into `build-<id>/bin/` and that services exec `current/bin/*` directly. A
new binary that is not added to that list is simply never deployed — and the
same page records think3/think4 running a stale release for hours because a
delivery path was assumed rather than verified. A library crate has no delivery
step at all.

This also keeps the change free of deployment work: no new port, no new systemd
unit, no new Nix option, so `homelab/modules/vibe-kanban-rebuild.nix` is
untouched.

---

## R9. Pre-existing CI gap, fixed in passing

The CI **`backend`** path filter (that is the filter's output name at
`.github/workflows/test.yml:58`; it gates the `backend-test`, `backend-clippy`,
and `backend-schema-checks` jobs) enumerates crates explicitly
(`.github/workflows/test.yml:59-69`): `api-types`, `db`, `deployment`,
`executors`, `git`, `local-deployment`, `remote`, `review`, `server`,
`services`, `utils`.

`crates/worker` and `crates/cluster-protocol` — both added by
`957e-clustered-vibe-k` — are **absent**. A change touching only those crates
does not trigger the Rust job, so `crates/worker/tests/restart_and_replay.rs`
does not run on its own changes.

This change adds `crates/node-metrics` and the two missing entries. Adding only
our own crate would leave the trap in place for the next person, and
`docs/knowledge-base/worktree-formatting-prerequisites.md` names this exact
false-pass mode.

`packages/ui/**` is already covered by the separate `frontend` filter, so the
frontend tasks need no filter change.

---

## Dependency summary

**New third-party dependencies: none.** `crates/node-metrics` uses `serde`,
`serde_json`, `chrono`, `thiserror`, `tokio`, `tracing`, `ts-rs`, and `uuid` —
all already workspace dependencies.
