# Coordinator NFS I/O pressure: analysis (2026-09-26)

Task `vk/78a5-analyze-and-redu`. This document explains why the coordinator
(think2, 6 cores) sat at load 20–50 with mostly idle CPU, what fixed it, and
how to measure it again.

## Method

All numbers were taken on think2 unless noted.

- `scripts/nfs-io-baseline.sh [seconds]` (read-only) reports load, the mean
  D-state task count (0.2 s polling), git processes spawned by the server
  (a lower bound: very short processes are missed), `/proc/pressure/{io,cpu}`,
  and per-op NFS rates and RTT for the shared mount from
  `/proc/self/mountstats`.
- `node scripts/time-workspace-summaries.mjs` times one
  `POST /api/workspaces/summaries` per archive scope against the coordinator's
  direct address.
- Git command lines were sampled every 0.5 s for 20 s
  (`ps -eo pid,ppid,stat,args`) and grouped by subcommand and workspace.

## Before (05:53–05:58 UTC, normal use, agents running)

| Measure | Value |
| --- | --- |
| Load 1/5/15 | 21.1 / 20.9 / 20.8 (peaks of 34–49 earlier) |
| CPU | ~80% idle |
| Mean D-state tasks | 8.5 |
| io PSI `some avg60` | 0.00–2.17 |
| NFS GETATTR | **24,456 ops/s** (ACCESS 269/s; every other op < 4/s; RTT 0.6–0.9 ms) |
| git spawns by the server | **≥ 775 / min** |
| summaries, active scope | **43.0 s**, 198 rows, 137 with stats computed |
| summaries, archived scope | **33.8 s**, 909 rows, 484 with stats computed |

The same script at 06:2x UTC, with no client polling, gave **0 git spawns,
960 GETATTR/s, load 2.4, and 0 D-state tasks**. The load follows open clients.

## Attribution

In the 20 s git sample, 137 processes came from the server. They were 85
`status --porcelain -z --untracked-files=normal`, 37 `read-tree HEAD`, 9
`diff --cached -M --name-status` and 1 `add -A`. Those four commands are the
steps of `GitCli::diff_status`, and they covered about 70 different workspaces.
Only one caller runs `diff_status` across many workspaces:

`POST /api/workspaces/summaries` → `compute_diff_stats` → `GitService::get_diffs`
(`crates/server/src/routes/workspaces/workspace_summary.rs`).

`useWorkspaces` (web-core) polls **both** the active and the archived scope
every 15 s on every open client. Each request took longer than 15 s (43 s and
34 s), so React Query refetched as soon as the previous request settled. The
result was that every open tab or device kept two full sweeps running
continuously, each four git jobs wide, with no sharing between clients or
scopes. On NFS, each `git status` lstats every tracked file, and each lstat is
one GETATTR round trip. That produced the GETATTR flood and the D-state
backlog. It also delayed unrelated NFS reads, such as raw logs for chat
history, which is the #326 symptom.

### Why io PSI did not show it

NFS client waits count as D state, so they raise the load average, but they are
not block-I/O stalls, so `/proc/pressure/io` stayed near zero. Node metrics now
report `procs_blocked` alongside io PSI. Server Metrics' CPU panel shows both
and flags blocked tasks greater than or equal to the core count.

### Hypotheses checked and rejected

- **NFS client options.** Recorded below and left unchanged. GETATTR volume
  scales with the number of `git status` walks, not with a mount defect.
  `nocto` or a long `actimeo` would let the coordinator read stale index and
  lock state written by workers. `nconnect` adds parallelism, not fewer round
  trips. `fsc` caches data only (READ ran at 0.5 ops/s), so it is neutral.
  Changing any option means a coordinated remount, which the homelab module
  deliberately prevents during switch.
- **Other coordinator scanners** (`find /srv/src/homelab`, git-projects
  stamping, the deploy loop). They do not touch the shared mount and did not
  appear in the D-state samples.
- **Raw-log re-normalization.** Logs live on NFS
  (`…/cluster/execution-logs`), but READ ran at 0.5 ops/s. They are a victim of
  the pressure, not a cause.

### NFS mount options (coordinator and workers are identical)

`172.16.0.99:/var/nfs/shared/VibeKanban` on `/srv/vibe-kanban-shared`:
`rw,relatime,vers=3,rsize=1048576,wsize=1048576,namlen=255,hard,proto=tcp,
timeo=600,retrans=2,sec=sys,fsc,local_lock=none`. That means default attribute
caching, close-to-open on, and no `nconnect`. Source:
`modules/vibe-kanban-rebuild.nix` in homelab, checked against live
`/proc/mounts` on think2 and think5. The export side (172.16.0.99) is not
reachable with the cluster service account and was not measured.

## Fix

`services::workspace_diff_stats::WORKSPACE_DIFF_STATS` is a process-wide
diff-stats cache used by the summaries route:

- **Single-flight per workspace.** Concurrent requests share one computation.
- **Process-wide bound.** At most 4 computations run at once, however many
  requests are open.
- **Staleness bounds.**

  | Workspace state | Staleness bound |
  | --- | --- |
  | Latest process running | **30 s** |
  | Idle (active) | **5 min** |
  | Archived | **60 min** |
  | Idle > 14 days | never computed in bulk (unchanged) |

- **Invalidation.** Any execution-process exit, local or remote, drops the
  workspace's entry. A computation that started before the exit never stores
  its result. The bounds above therefore apply only to changes made outside
  Vibe Kanban processes, such as a manual edit or a UI-driven rebase or merge.
- **Failures are not cached.** A partial result (some repo’s git step failed) is
  served but retried after at most 60 s.
- **Cancellation-safe.** The computation runs in its own task that holds the
  slot lock and the permit until its blocking git work ends, so an aborted
  request cannot start duplicates or exceed the bound.
- The stat semantics are unchanged. The diff view and branch status stay live.

Known gap: the two invalidation call sites in `LocalContainerService`
(the local exit monitor and `finalize_remote_execution`) are not unit-tested.
The cache's `invalidate` contract is tested, and the TTL bounds a missed
invalidation.

Expected effect: with N clients, git work drops from 2·N continuous sweeps to
a recompute of only the workspaces whose window has expired. That is about 137
active workspaces every 5 min plus running ones every 30 s, and archived ones
hourly. Summary requests for fresh workspaces return without touching git.

Follow-ups, not done here: an ntfy alert when `procs_blocked ≥ cores` persists;
fetching archived summaries only when the archived section is open; event-driven
stats pushes.

## After

To be recorded after deploy, using the same commands, with one or two clients
open and agents running:

| Measure | Before | After |
| --- | --- | --- |
| Load 1/5/15 | 21.1 / 20.9 / 20.8 | _pending deploy_ |
| Mean D-state tasks | 8.5 | _pending_ |
| NFS GETATTR ops/s | 24,456 | _pending_ |
| git spawns / min (lower bound) | 775 | _pending_ |
| summaries active / archived (warm) | 43.0 s / 33.8 s | _pending_ |
