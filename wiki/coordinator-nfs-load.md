# Coordinator NFS load: diagnosing it and keeping bulk git work shared

Every workspace worktree lives on NFSv3 (`/srv/vibe-kanban-shared`). On NFS,
each `lstat` in `git status` is a GETATTR round trip, so work that walks many
worktrees costs round trips, not CPU. Full numbers:
`docs/analysis/coordinator-nfs-io-pressure.md`.

## Signature, and the metrics that miss it

- A load far above the core count, idle CPU, and many `git -C
  /srv/vibe-kanban-shared/…` processes mean NFS wait.
- **`/proc/pressure/io` stays near 0 and so does `/proc/stat` `procs_blocked`.**
  Both count only `io_schedule` waiters. NFS RPC waits put tasks in D state
  and raise the load without counting as I/O waiters.
- Count D-state tasks directly. Count **threads**, not processes:
  `/proc/[pid]/stat` shows only the leader's state, and the server blocks its
  `spawn_blocking` workers. `node-metrics` reports this as
  `cpu.uninterruptible_tasks` (it walks `task/*/stat` for multithreaded
  processes), and the CPU panel flags it when it reaches the core count.
- Tools: `scripts/nfs-io-baseline.sh 60` on think2 gives per-op NFS rates
  from `/proc/self/mountstats`, git spawns by the server, D-state tasks and
  PSI. `node scripts/time-workspace-summaries.mjs` times the summaries
  endpoint directly. The service account can `ssh think2` from any worker.

## Root cause found (2026-09-26)

`useWorkspaces` polls `POST /api/workspaces/summaries` for **both** the active
and the archived scope every 15 s on every client. Each workspace ran
`GitCli::diff_status` (read-tree, status, add, diff) plus blob reads. A
request took 34–43 s, longer than the poll interval, so each open tab kept two
sweeps running without pause. The result was about 24k GETATTR/s and load
about 21. With no client open: 0 git spawns and load 2.4. **Load on the
coordinator scales with open clients.** Look there first.

## The rule: bulk scans go through a shared single-flight cache

`services::workspace_diff_stats::WORKSPACE_DIFF_STATS`:

- One slot per workspace. The leader computes and joiners wait on the slot
  lock. A process-wide 4-permit semaphore bounds the work across all requests,
  never per request.
- **Tiered staleness.** 30 s while a process is running, 5 min when idle,
  60 min when archived. Any execution-process exit invalidates the entry,
  through both the local exit monitor and `finalize_remote_execution`. Agents
  usually run on workers, so the remote path is the one that matters.
- **Generation-based invalidation.** A computation that started before an
  invalidate never publishes its result.
- **The leader runs in its own `tokio::spawn`,** owning the `OwnedMutexGuard`
  and `OwnedSemaphorePermit`. Gotcha: `spawn_blocking` work continues after
  the awaiting future is dropped. If the lock and permit live in the request
  future, a cancelled request frees them while its git processes keep
  running, which breaks both single-flight and the concurrency bound.
- **Partial results.** `compute_diff_stats` skips a repo whose base lookup or
  diff fails. `compute_diff_stats_outcome` reports `complete: false` for that
  case. Such a result is served, but it is retried within 60 s instead of
  being cached for the whole tier. `None` is never cached.

## Bound the response, not just the work (2026-09-29)

The cache bounds the **work**. It does not bound **one request's latency**.
The summaries handler still awaited every workspace's stats, so each row's
cheap metadata (process status, unseen turns, PR, affinity) waited on the
slowest `git status`. Measured from a worker: 0.27 s, then 16.6 s, then 1.8 s
for 204 active rows. On mobile the rows rendered blank.

- `DiffStatsCache::get_within(id, max_age, deadline, compute)` waits at most
  `deadline`. If the fresh value misses it, the call returns the slot's
  `last_known` stats (`None` if never computed). The spawned leader keeps
  running and publishes, so the next poll is a hit. Missing the deadline
  starts no extra compute, and a caller queued behind a leader just reads the
  fallback.
- `last_known` lives in a sync mutex beside the async `state` lock, because
  a leader holds `state` for the whole computation. It is written on every
  successful compute, **including one invalidated mid-flight**, and it survives
  `invalidate`. It is a display fallback, never "fresh": a recompute that
  fits the deadline always wins.
- A compute that **finished and failed** still returns `None`, not
  `last_known`. A failure can mean the worktree is gone, and old numbers
  would mislead.
- The budget is **request-wide** (`SUMMARY_DIFF_STATS_BUDGET` = 3 s, one
  deadline taken before `buffer_unordered`). A per-item timeout would still
  add up to about N/concurrency × timeout. Items that reach the buffer after
  the deadline get a zero duration. `tokio::time::timeout` polls the inner
  future once first, so an immediate fresh hit is still served.

Rejected alternatives: tuning NFS mount options (`nocto` or a long `actimeo`
breaks cross-host coherency; homelab principle 120), a background refresher
(does work when nobody is looking), and client-side throttling alone (N
devices still multiply the work).

## Contributed by

- vk/78a5-analyze-and-redu
- vk/b923-workspaces-loadi
