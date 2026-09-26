# Implementation plan: coordinator NFS I/O pressure (vk/78a5-analyze-and-redu)

See `SPEC.md` for the measured attribution. Steps are in dependency order. Each
step lands with its tests.

## 1. Shared diff-stats cache (services)

File: `crates/services/src/services/workspace_diff_stats.rs` (new; register it in
`services/mod.rs`).

- `pub struct DiffStatsCache { slots: DashMap<Uuid, Arc<Slot>>, permits: Arc<Semaphore> }`
  where `Slot { state: tokio::sync::Mutex<Option<Entry>>, generation: AtomicU64 }` and
  `Entry { stats: DiffStats, computed_at: tokio::time::Instant }`.
- `get_or_compute(id, max_age, compute: impl Future<Output = Option<DiffStats>>)`:
  1. Lock the slot's state. If `Some(entry)` has `computed_at.elapsed() <= max_age`,
     return it (hit).
  2. Snapshot `generation`, take a permit from `permits`, and await `compute`.
  3. If the result is `Some` **and** the generation is unchanged, store it.
     Return the result either way. `None` is never stored, so a failure clears
     coordination.
  - Callers that wait on the slot lock join the leader (single-flight).
- `invalidate(id)`: `generation.fetch_add(1)` and `try_lock`-clear the entry. If the
  slot is busy, the generation bump alone prevents the in-flight write-back, and
  the next reader recomputes because the stored entry is also dropped when
  `generation` differs from the one it was stored under. Store `generation` in the
  `Entry` and treat a mismatch as a miss.
- `prune(older_than)`: drop idle slots whose entry is older than the longest tier.
  Call it from `get_or_compute` at most once a minute, which bounds retained state.
- Tier enum `DiffStatsFreshness { Running, Idle, Archived }` with
  `max_age()` = 30 s / 5 min / 60 min, plus `fn freshness_for(archived, latest_running)`.
- `pub static WORKSPACE_DIFF_STATS: LazyLock<DiffStatsCache>` (a `LazyLock`
  static, the existing idiom in `services::container`), with
  `BULK_DIFF_STATS_CONCURRENCY = 4`.
- Tests (`#[tokio::test(start_paused = true)]`): hit within max_age; recompute
  after expiry; 8 concurrent callers → 1 compute; invalidate → recompute;
  invalidate during an in-flight compute → result not stored; `None` not cached;
  permits bound concurrency across different ids; tier mapping.

## 2. Summaries route uses the cache (server)

File: `crates/server/src/routes/workspaces/workspace_summary.rs`.

- Replace the direct `compute_workspace_diff_stats` in step 8 with
  `WORKSPACE_DIFF_STATS.get_or_compute(ws.id, freshness.max_age(), compute_…)`,
  where `freshness = DiffStatsFreshness::for_workspace(ws.archived, latest.status == Running)`.
- Keep the 14-day idle skip and `MAX_CONCURRENT_GIT_STATUS` for the stream
  fan-out. The global semaphore now bounds actual git work process-wide.
- Unit test for freshness selection from `LatestProcessInfo`.

## 3. Invalidate on process exit (local-deployment)

File: `crates/local-deployment/src/container.rs`, in the exit-monitor finalization
block (next to the CodingAgent remote-sync `compute_diff_stats`).

- Call `WORKSPACE_DIFF_STATS.invalidate(ctx.workspace.id)` for every finished
  execution process, before the remote sync.

## 4. I/O pressure metrics (node-metrics → UI)

- `parse.rs`/`collect.rs`: parse process `state` and count `D` processes in the
  `/proc/[pid]` walk (`/proc/stat` `procs_blocked` is only `nr_iowait` and misses
  NFS waits), and `parse_pressure(&str) -> Option<Pressure { some_avg60, full_avg60 }>`
  (`/proc/pressure/io`). Fixture-based tests, including a missing file and
  garbage input returning `None`.
- `types.rs` `CpuSample`: add `#[serde(default)] uninterruptible_tasks: Option<u32>`,
  `#[serde(default)] io_pressure_some_avg60: Option<f32>`,
  `#[serde(default)] io_pressure_full_avg60: Option<f32>`.
- `collect.rs`: populate them. A missing `/proc/pressure/io` (kernel without PSI)
  adds a `degraded` note and yields `None`, never `0`.
- Fix up every `CpuSample { … }` literal (tests in services/cluster/metrics.rs etc.).
- `pnpm run generate-types`.
- `CpuPanel.tsx`: add rows "Blocked tasks (D state)" and "I/O pressure (some / full,
  60 s)". The blocked row is marked with a warning tone when `uninterruptible_tasks >= core_count`.
  Add a Vitest test for the formatting/warning helper.

## 5. Analysis document

`docs/analysis/coordinator-nfs-io-pressure.md`: methodology (the sampling script,
committed as `scripts/nfs-io-baseline.sh`), before numbers, attribution, rejected
hypotheses, NFS options on coordinator and workers, staleness bounds, and after
numbers captured post-deploy.

## 6. Verify

`cargo test -p services -p node-metrics -p server` (touched crates), `pnpm run check`,
`pnpm run lint`, `pnpm run format`. Deploy, then rerun
`scripts/nfs-io-baseline.sh 60` on think2 plus the timed summaries probe, and record
the after numbers.
