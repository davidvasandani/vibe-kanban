# SPEC: Reduce NFS I/O pressure on the coordinator (think2)

## Problem

The coordinator (think2, 6 cores) runs at a sustained load average of 20–50
while its CPU is 40–80% idle. The load is tasks blocked on NFS round trips
against `/srv/vibe-kanban-shared` (`172.16.0.99:/var/nfs/shared/VibeKanban`,
NFSv3, `fsc`). Users see it as slow sidebars, slow diff and summary
responses, and log sockets that close without `finished` (the trigger for the
spinner fixed client-side in #326).

## Measured attribution (2026-09-26, 05:53–05:58 UTC, think2)

| Measure | Value |
| --- | --- |
| Load average (1/5/15) | 21.1 / 20.9 / 20.8 (6 cores) |
| Mean tasks in D state (0.2 s sampling, 60 s) | 8.5 |
| `/proc/pressure/io` `some avg60` | 0.00–2.17 (NFS RPC waits are **not** accounted as block I/O pressure) |
| `/proc/pressure/cpu` `some avg60` | 8.9–26.6 |
| NFS GETATTR from think2 | **24,456 ops/s** (ACCESS 269/s; every other op < 4/s) |
| git processes spawned by the server (lower bound, 0.2 s sampling) | **775 / min** |
| git command mix in a 20 s sample (137 processes) | 85 `status --porcelain -z --untracked-files=normal`, 37 `read-tree HEAD`, 9 `diff --cached -M --name-status`, 1 `add -A` — all four steps of `GitCli::diff_status`, across ~70 distinct workspaces |
| `POST /api/workspaces/summaries {archived:false}` | **43.0 s**, 198 workspaces, 137 with git diff stats computed |
| `POST /api/workspaces/summaries {archived:true}` | **33.8 s**, 909 workspaces, 484 with git diff stats computed |

Attribution: the only caller that runs `diff_status` across many workspaces
at once is `get_workspace_summaries` → `compute_workspace_diff_stats` →
`diff_stream::compute_diff_stats` → `GitService::get_diffs`. Every other
caller of these paths is scoped to one workspace: the open diff stream, the
branch-status route, turn finalization, and a remote sync after login.

Every open client (`useWorkspaces`) polls both the active and the archived
summaries every 15 s. Each request takes longer than the interval (43 s and
34 s), so React Query starts the next refetch as soon as the previous one
settles. **Every open client therefore keeps two full git sweeps running
continuously**, each at `MAX_CONCURRENT_GIT_STATUS = 4`. The server never
shares or coalesces the work across clients or across the two scopes.

Each workspace-repo in a sweep costs, on NFS:
`git2::Repository::open` + merge-base, `read-tree HEAD` into a temp index,
`status --porcelain --untracked-files=normal` (lstat of every tracked file →
one GETATTR each), `add -A` of the changed paths, `diff --cached -M`, then
in-process blob and file reads for line counts.

Refuted or secondary hypotheses:

- **NFS client settings.** GETATTR volume is proportional to the number of
  `git status` walks, not a mount-option defect. Raising `actimeo` or adding
  `nocto` would trade the cross-host freshness that workers rely on (index
  and lock files written on one host, read on another) for fewer round trips.
  That is rejected while an app-side fix removes the walks themselves.
  `fsc` only caches file *data* (READ is 0.5 ops/s), so it neither helps nor
  thrashes this workload. No mount change is made.
- **Other coordinator scanners** (`find /srv/src/homelab`, git-projects
  stamping, the deploy loop). They are not on the shared mount and did not
  appear in the D-state samples.
- **Raw-log re-normalization.** Raw logs live under
  `/srv/vibe-kanban-shared/cluster/execution-logs` (NFS), but history reads
  are per request and bounded by `HISTORICAL_NORMALIZATION_PERMITS`. They
  produced no measurable GETATTR/READ volume in the samples (READ 0.5 ops/s).

## Goals

1. Workspace diff stats in bulk summaries are computed at most once per
   workspace per staleness window, however many clients or requests ask.
2. Concurrent requests for the same workspace share one computation
   (single-flight).
3. Total bulk diff-stat git work is bounded process-wide, not per request.
4. Staleness is explicit and bounded, and stats are invalidated immediately
   when a process in the workspace finishes (the moment an agent's edits land).
5. Node metrics expose NFS-relevant pressure (blocked tasks plus io PSI), so
   the Server Metrics UI shows this before users see spinners.

## Non-goals

- Changing NFS mount options, fscache, or the NFS export (see above). The
  analysis records the current options; no host config change is made.
- Changing the single-workspace paths (diff stream, branch status). They
  stay live and uncached.
- Changing how diff stats are computed (their semantics stay identical).

## Design

### Shared diff-stats cache (`services::services::workspace_diff_stats`)

A process-wide `LazyLock` cache keyed by workspace id. Each slot is a
`tokio::sync::Mutex<Option<Entry { stats, computed_at, generation }>>`:

- `get_or_compute(id, max_age, compute)`: lock the slot. If the entry is
  younger than `max_age`, return it. Otherwise acquire a permit from a
  process-wide semaphore (`BULK_DIFF_STATS_CONCURRENCY = 4`), compute,
  store, and return. Waiters on the same slot get the fresh value
  (single-flight).
- `invalidate(id)`: bump a generation counter and drop the entry. A
  computation that started before the bump does not publish its result (no
  stale write-back race).

Freshness tiers, chosen by the summaries route:

| Workspace state | `max_age` (staleness bound) |
| --- | --- |
| Latest process running | 30 s |
| Active, idle | 5 min |
| Archived | 60 min |
| Idle > 14 days | not computed (unchanged behaviour) |

Any process completion in the workspace invalidates immediately, so an
agent's finished turn shows up on the next poll.

### Invalidation hook

`LocalContainerService` finalization calls `invalidate(workspace_id)` when any
execution process in the workspace exits. That covers coding agent turns,
setup, cleanup and dev scripts, and it runs before the turn's remote sync.

### Metrics: I/O pressure

`node_metrics::CpuSample` gains optional `uninterruptible_tasks` (D-state processes counted from the `/proc/[pid]` walk; `/proc/stat` `procs_blocked` is only `nr_iowait`)
and `io_pressure_some_avg60` / `io_pressure_full_avg60` (from
`/proc/pressure/io`). Both are optional and `#[serde(default)]`, so mixed
versions still interoperate. The Server Metrics node view shows
"blocked tasks N · io some/full X%" next to the load, and warns when `uninterruptible_tasks`
is at or above the core count.

## Acceptance

- Analysis with before/after numbers is recorded in
  `docs/analysis/coordinator-nfs-io-pressure.md`. The before figures are the
  table above. The after figures are measured with the same script after
  deploy.
- Unit tests: cache hit within `max_age`, recompute after expiry,
  single-flight (N concurrent callers → one compute), invalidation drops the
  entry and blocks stale write-back, tier selection, and the `/proc/stat` and
  `/proc/pressure/io` parsers.
- Summary fields and diff-stat semantics are unchanged. Only freshness
  changes, within the stated bounds.
