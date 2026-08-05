# Analysis: Cross-artifact and constitution check

Artifacts checked: `spec.md`, `clarifications.md`, `plan.md`, `research.md`,
`data-model.md`, `contracts/worker-metrics.md`,
`contracts/coordinator-metrics-api.md`, `tasks.md`, against
`.specify/memory/constitution.md` v0.17.0.

Every file:line claim in the artifacts was re-verified against the source. The
findings below are the ones that survived that verification; each `error` was
independently confirmed by reading the cited code.

**Status: 7 errors, 10 warnings — all resolved in the artifacts. See
"Resolutions" at the end.**

---

## Errors

### E1 — The FR-24 mechanism writes lifecycle state (violates FR-21 and Constitution XIX)

**Artifacts:** `plan.md` Layer 3; `contracts/coordinator-metrics-api.md`;
`tasks.md` T020.

The plan called `WorkerRegistry::expire_heartbeats(now)` before every metrics
node listing, copying `list_workers`. That is not a read:

- `crates/services/src/services/cluster/registry.rs:140-142` → `expire_leases`
- `crates/db/src/models/worker_node.rs:160-173`:
  `UPDATE worker_nodes SET status = 'offline', updated_at = ... WHERE status = 'online' AND (lease_expires_at IS NULL OR lease_expires_at <= ?)`

Constitution XIX: *"No observability path may write scheduling, liveness, lease,
eligibility, or lifecycle state."* FR-21 says the same. As written, whether a
worker transitions to `offline` — and when — would depend on whether an operator
happens to have a monitoring drawer open, at up to 0.5 Hz. That is precisely the
coupling the principle exists to prevent.

It also silently defeated T022: that task asserts the fields are unchanged
across a *failed poll*, so it would pass while the *success* path mutated rows
every tick.

**Fix applied:** no metrics path calls `expire_heartbeats`. Health is derived
in memory for presentation (`lease_expires_at <= now → offline`). T022 gains a
case asserting `status` and `updated_at` are unchanged across a *successful*
snapshot build.

### E2 — FR-28's replay requirement is not met, and the artifacts claimed a nonce that does not exist

**Artifacts:** `plan.md` Layer 2; `contracts/worker-metrics.md`.

Both claimed `GET /v1/metrics` inherits nonce-based replay protection. It does
not. `require_signature` (`crates/worker/src/worker_api.rs:409-463`) checks only
the timestamp, the signature, and the body digest. The nonce map is consulted
exclusively in `validate_authority` (`:386-407`), which is payload-level and
only runs for routes carrying a `RequestAuthority` in a JSON body — and the plan
itself states two sentences later that this route carries none. The client side
confirms it: `signed()` (`crates/services/src/services/cluster/client.rs:335-353`)
emits only `x-vk-timestamp`, `x-vk-content-sha256`, `x-vk-signature`.

A captured request is therefore replayable verbatim for up to 30 seconds.
T018's four cases contained no replay test — because one would have failed.

**Fix applied:** the claim is removed, FR-28 is amended to state the actual
guarantee honestly, and the residual is documented. Adding a nonce header would
change shared transport code used by every `/v1/*` route; that blast radius is
not justified to protect a read-only, already-authenticated, 30-second-window
replay of a metrics fetch. Recorded as an accepted residual with the condition
that reopens it.

### E3 — The contract documented the wrong `x-vk-timestamp` format

**Artifact:** `contracts/worker-metrics.md`.

Documented as RFC3339. It is Unix epoch seconds as a decimal string — the worker
parses `.parse::<i64>()` (`worker_api.rs:416-420`) and the client emits
`Utc::now().timestamp().to_string()` (`client.rs:346`). An implementer writing a
client from this contract would get a blanket `401` with no diagnosis.

**Fix applied:** corrected.

### E4 — The root JSON Patch cannot be applied by the client hook this design depends on

**Artifacts:** `data-model.md`; `contracts/coordinator-metrics-api.md`;
`plan.md` Layer 4; `tasks.md` T023/T026.

The entire level-triggered resnapshot story rested on `replace` at path `""`.
The consuming hook applies patches by mutating an Immer draft
(`packages/web-core/src/shared/hooks/useJsonPatchWsStream.ts:33`); for `path: ""`
rfc6902's pointer yields `parent === null`, so `replace` returns `MissingError`,
`applyUpsertPatch` retries it as `add`, and that attempts `null[''] = value`.
Best case a silent no-op, worst case a `TypeError` swallowed into
`setError('Failed to process stream update')`. Either way the snapshot never
lands and the panel never converges — the backstop would have been dead on
arrival.

The repo's own idiom confirms the point: the approvals snapshot uses a named
sub-path (`crates/services/src/services/events/patches.rs:160-165`), not root.

**Fix applied:** resnapshot emits `replace /nodes` plus `replace /generated_at`,
with `initialData()` seeding `{ nodes: {}, generated_at: null, sample_interval_ms }`.

### E5 — FR-7 and FR-5 were contradicted by the data model and by the test meant to cover them

**Artifacts:** `spec.md` FR-5/FR-7; `data-model.md`; `tasks.md` T012.

Two independent problems:

1. T012 asserted "first-sample-has-no-rates (**all rates `0`**)" — exactly what
   FR-7 forbids and what Constitution XIX calls prohibited ("A zero that means
   'no reading' is prohibited"). The same defect appeared in `data-model.md`'s
   `NetworkSample` note.
2. The types made the requirement *inexpressible*: `total_busy_percent: f32`,
   `per_core_busy_percent: Vec<f32>`, `rx_bytes_per_second: u64`,
   `cpu_percent: f32`, `load_*: f32`, and every `MemorySample` field were
   non-`Option`. There was no representation for "not yet available", so an
   unreadable `/proc/meminfo` could only be reported as `0` — violating
   invariant 2 in the document that states it.

**Fix applied:** every rate-derived and every possibly-unreadable field is
`Option<T>`; T012 asserts `None`; the "yields `0`" language is deleted.

### E6 — FR-24 was unsatisfiable: `MetricsNode` carried no health field, and Offline workers were never polled

**Artifacts:** `data-model.md`; `plan.md` Layer 3.

`MetricsNode` had no `WorkerNodeStatus`, `mount_status`, or `lease_expires_at`,
and `availability` is explicitly *metrics* availability, not health. The drawer
therefore could not display node health at all, let alone match Settings.

Compounding it, "polls all non-`Offline` workers" means a lease-expired worker
is never polled, so its `availability` freezes at its last value — plausibly
`available`. That is the exact inversion FR-24 forbids: healthy here, dead in
Settings.

**Fix applied:** `MetricsNode` gains a `health` field sourced (read-only, per
E1) from the worker row; `NodeStrip` renders it; T022 gains a lease-expired case.

### E7 — The coordinator node had no defined identity when clustering is disabled

**Artifacts:** `data-model.md`; `tasks.md` T020.

`ClusterConfig.coordinator_id` is `Option<Uuid>` and defaults to `None`
(`crates/services/src/services/cluster/config.rs:41, :53`); it is only required
when `enabled`. But `nodes` is keyed by `Uuid` and `MetricsNode.node_id` is
non-optional, and **no artifact said what to use**. This is the spec's *first*
acceptance criterion.

Separately, `ClusterConfig` has no hostname field and `HostSample` had none
either, so `MetricsNode.hostname` (required by FR-11) had no specified source
for any node.

A naive `Uuid::new_v4()` per boot would also break the persisted
`selectedNodeId` (FR-13) on every restart.

**Fix applied:** the coordinator id is a UUIDv5 over a fixed namespace and the
hostname — stable across restarts, no config required; `HostSample` gains
`hostname`; T022 gains a `enabled: false → exactly one node` case.

---

## Warnings

| # | Artifact | Finding | Resolution |
| --- | --- | --- | --- |
| W1 | `data-model.md` | Constitution XIX names **stale** as a distinct status; the enum had only `Available`/`Unsupported`/`Unreachable`/`NotImplemented`, forcing the UI to infer staleness from `last_contact_at` arithmetic. No state existed for "never collected", which is the normal REST-fallback condition | Added `Stale { since }` and `NotCollected` |
| W2 | `contracts/coordinator-metrics-api.md` | The REST "fallback" is not one: with the collector subscriber-gated and readings expiring at 5 min, it returns `latest: null` for every worker whenever the drawer has been shut a while | Documented honestly, and the REST path now triggers one bounded collection round |
| W3 | `data-model.md`, contract | The patch scheme assumed exactly one new sample per node per tick. Sampler and poller are independently phased: a tick routinely yields 0 or 2 samples, and the first poll after connect yields up to 150 | Specified one append+evict *per sample received*; zero-sample tick emits nothing. T024 gains both cases |
| W4 | `tasks.md` T033 | FR-20's error semantics were assigned to the container, but `useJsonPatchWsStream` owns them and never reports an error once any data has arrived (`:170-195`). The container cannot override without changing a hook shared by approvals, raw/normalized logs, scratch, and browser sessions | FR-20 weakened to match the hook; the alternative is recorded with its blast radius |
| W5 | contract | No ping/pong, idle timeout, or ceiling on `subscriber_count`. A half-open socket pins the collector on indefinitely while the reconnecting client increments a *new* subscriber — FR-16 fails open | Added a server-side liveness deadline |
| W6 | `tasks.md` T031/T032 | Constitution IV scopes `packages/ui` to **primitives**. The feature panels are not primitives, and `MetricsDrawer` was given stateful behaviour plus a dependency on a `web-core` zustand store — verified impossible: `packages/ui/package.json` has neither `zustand` nor `@vibe/web-core` | Panels moved to `web-core`; `MetricsDrawer` is props-only; `Meter`/`Sparkline` stay in `ui` as genuine primitives |
| W7 | `tasks.md` | No task added `node-metrics` to `crates/services/Cargo.toml` or `crates/server/Cargo.toml`; T038 would fail to compile | Added |
| W8 | `spec.md` FR-8 vs `tasks.md` T034 | "reachable from anywhere" vs. a wrapper that unmounts the feature on mobile, with no Out-of-Scope entry | Mobile added to Out of Scope |
| W9 | `tasks.md` | Ten acceptance criteria had no covering task or test: btop agreement, disk/NFS, FR-13 persistence, FR-14 a11y, FR-19 malformed-data isolation, FR-18 UI half, FR-26 logging path, FR-28 replay, FR-24 health parity, FR-2 clustering-disabled | Tasks added for each |
| W10 | `data-model.md` vs contract | `NodeRole` serialised as `"Coordinator"` in one and `"coordinator"` in the other; the frontend comparison would silently never match | `rename_all = "snake_case"` specified |

---

## Info

- The CI filter is named **`backend`**, not `rust`
  (`.github/workflows/test.yml:41,58`); jobs are `backend-test` /
  `backend-clippy` / `backend-schema-checks`. The line range and the
  missing-crate finding were correct — only the name was wrong. Corrected in
  `research.md` R9 and T003. `packages/ui/**` is already covered by the
  `frontend` filter.
- `crates/server/src/routes/approvals.rs` does **not** use `MsgStore`; it drives
  `deployment.approvals().patch_stream()` straight onto the socket. Only the
  `LogMsg::JsonPatch` half of the cited idiom was accurate. Corrected.
- `docs/knowledge-base/electric-sync-fallback.md` does not exist; the page is
  `wiki/electric-sync-fallback.md` — cited correctly six lines later in the same
  document. Corrected.
- `SampleBatch` history entries carrying `processes: []` are indistinguishable
  from "no processes readable" — a soft violation of invariant 2. Changed to
  `Option<Vec<ProcessSample>>`.

## References verified correct (no action)

`worker_api.rs:89-114`, `:409-463`, `:449-454`, `:258`; `client.rs:279-289`,
`:63`; `workers.rs:33-37`, `:53-57`; `routes/mod.rs:63-64`, `:99`;
`signed_ws.rs:33`; `scheduler.rs:80-90`; `Cargo.toml:44`; migration `:32`;
`cluster-protocol/src/lib.rs:46-52`, `:54-63`; `worker/src/server.rs:44-170`,
`:320-347`; `config.rs:19`; `generate_types.rs:75-77`; `MobileDrawer.tsx:12`;
`ContextUsageGauge.tsx`; `useJsonPatchWsStream.ts:33`; `useOrgRailStore.ts:17`;
`api.ts:197`; `SharedAppLayout.tsx:446`; `actions/index.ts:621`, `:1561`; the
"no charting library" and "no new dependency" claims.

---

## Resolutions

All 7 errors and 10 warnings are corrected in the artifacts. The two findings
worth carrying forward as *decisions* rather than fixes:

1. **E2 (replay window).** `GET /v1/metrics` is replayable within the 30s
   timestamp drift window, like every other bodyless `/v1/*` GET
   (`/v1/jobs`, terminal output, event fetches). Closing it means adding a nonce
   header to shared transport code used by every worker route. For a
   read-only, signature-authenticated fetch whose replay yields nothing the
   attacker could not get by replaying `/v1/jobs`, that blast radius is not
   justified. **Reopens if** a mutating bodyless route is ever added, at which
   point the nonce belongs in the transport layer for all of them.
2. **W4 (stream error reporting).** FR-20 is scoped to what
   `useJsonPatchWsStream` actually provides. Extending the hook to distinguish
   "recovered" from "degraded but has stale data" would improve every consumer,
   but it is a separate change with a five-consumer blast radius and does not
   belong in this task.
