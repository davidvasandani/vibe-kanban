# Workspace-list read path: caching per-workspace Git stats

How the workspaces sidebar got cheap, and the traps in the read path that made it
expensive. Applies to any VK surface that wants a per-workspace number derived
from a worktree.

## The shape of the problem: a derived value on a poll loop

`POST /api/workspaces/summaries` returns one row per workspace and is polled by
the client every 15 s (`refetchInterval: 15000`, `refetchOnMount: 'always'`) for
**both** the active and archived lists. Everything in that response is a cheap
batched DB read except three integers — `files_changed`, `lines_added`,
`lines_removed` — which require walking a worktree with `git`.

Measured on the cluster coordinator (506 workspaces, worktrees on NFSv3):

| Request | Rows | Latency |
| --- | --- | --- |
| `{"archived":false}` | 132 | 5.64 s, 5.78 s |
| `{"archived":true}` | 374 | 1.92 s, 1.67 s |

~7.5 s of git work every 15 s, per open tab, forever, whether or not anything
changed. The generalisable lesson: **a derived-from-disk value on a fixed poll
interval is a standing load, not a per-request cost.** Budget it as
`cost × rows × tabs / interval` before putting it in a list endpoint.

## Where the cost actually is (and is not)

The Rust code's shape suggests the expensive part is materialising diffs. It is
not. Timing raw `git` against one real worktree on the NFS mount:

| Stage | Cold cache | Warm (3 runs) |
| --- | --- | --- |
| Temp-index staging: `read-tree HEAD` + `status --porcelain -z` + `add -A` | 7.00 s | 1.52 / 1.65 / 1.48 s |
| `diff --cached -M --numstat -z <base>` | — | 0.044 / 0.057 / 0.055 s |

**The staging sweep is ~30× the diff.** `git status` `lstat()`s every tracked
path and `git add -A` re-hashes every changed file; on NFS that dominates. So:

- Switching from "materialise every `Diff` then sum" to `--numstat` is worth
  doing (it removes a `Repository::open`, a blob inflate, an `fs::read` of every
  changed file up to 2 MiB, and a `git2::Patch` Myers diff per file) but it is a
  **secondary** win.
- Caching is the primary win, because it removes the staging sweep from the
  request path entirely.
- Anyone optimising this further must attack staging — `core.fsmonitor`, an
  untracked cache, or `git add -N` plus a non-`--cached` diff. Do not expect
  gains from the diff command.

Note that `git add -A` writes loose objects into the **shared** object store even
though the index itself is a `/tmp` file, so this read path is a writer to
cluster-shared state. Reducing its frequency reduces that too.

### The temp index is load-bearing — do not "simplify" it away

`git diff --numstat <base>` on its own counts neither staged nor untracked
content. The staging preparation (`read-tree HEAD` → `status` → `add -A` under
`GIT_INDEX_FILE`) is what makes untracked files count, matching the porcelain
cleanliness definition rather than the `include_untracked(false)` one (see
[workspace-directory-reclamation](workspace-directory-reclamation.md)). Both the
`--name-status` and `--numstat` paths must share that preparation. There is a test
that asserts an untracked file is counted, specifically to fail if someone drops
it.

## Stale-while-revalidate for a value the server can't cheaply validate

The endpoint now reads an in-memory cache and schedules a background refresh:

```rust
let stats = deployment.workspace_diff_stats().snapshot(&ids).await;      // memory only
deployment.workspace_diff_stats().refresh_stale(pool, git, &workspaces).await; // spawns
```

Freshness comes from two mechanisms with clearly separated roles:

1. **Event-driven invalidation is the latency optimisation.** A watcher
   subscribes to the SQLite-update-hook JSON-patch stream and invalidates on any
   `/workspaces/{uuid}` add/replace patch. One hook covers the whole execution
   lifecycle for free, because a workspace patch is already pushed on every
   `execution_processes` insert/update. Model it on
   `BrowserSessionService::spawn_cleanup_watcher`: `Arc::downgrade` the inner
   state so dropping the service stops the task, and treat
   `RecvError::Lagged(_)` as `continue`.
2. **A TTL sweep is the correctness backstop.** The broadcast channel is lossy
   and a lagging subscriber silently skips events, so the TTL is not a tuning
   knob — it is the only thing that catches anything the server cannot observe
   (a file edited in an external editor, a worktree pruned by another
   workspace's cleanup). Keep it unconditional. Same level-triggered-over-edge-triggered
   discipline as [issue-status-side-effects](issue-status-side-effects.md).

Nothing self-drives the sweep: `refresh_stale` runs because a client polled. With
no client connected nothing refreshes, which is the desired idle cost — but it
means tests must not assume a background timer exists.

### Invariants worth copying

- **A failure is not a zero.** The pre-existing code skipped a failed repo and
  returned a zeroed struct, so a workspace whose worktree was gone reported a
  confident "0 files changed". Cache an entry **only** when every repo computed
  successfully; otherwise leave it absent so the field stays `None`. "A git probe
  that errors is not a clean repo."
- **A stale refresh must not clobber a fresher invalidation.** Capture a
  per-workspace generation before the git work and re-check it before inserting.
- **Bound every auxiliary map.** The moka cache has a TTL *and* a capacity, but
  the generation and in-flight maps are plain `DashMap`s with no eviction. Track a
  generation *only while a refresh is in flight* (with nothing in flight there is
  no result to poison), and clear the in-flight marker from a `Drop` guard so a
  persistently failing workspace is retried instead of pinned forever.
- **No `tokio::time::timeout` around `spawn_blocking`.** The usual repo idiom is
  bounded fan-out with per-probe timeouts, but a timeout cannot cancel a blocking
  task — it would release the semaphore permit while the thread stayed stuck on
  NFS, letting the fan-out grow without bound. Hold the permit for the whole
  blocking call and let the semaphore be the bound. The pre-existing code used an
  unbounded `join_all`, so one slow NFS server stalled all 132 workspaces at once.
- **Skip work that cannot produce an answer**, but do not trust the skip.
  `worktree_deleted` and `container_ref` are advisory — a repo-wide
  `git worktree prune` can remove a working tree while the columns survive — so
  they are a cheap filter, never proof a cached value is valid.

### A cold cache is invisible only because every consumer already tolerates it

`files_changed` and friends were **already** `Option<usize>` on the wire, and
every reader either `?? 0`s them or gates on `!== undefined`
(`hasChanges = filesChanged !== undefined && filesChanged > 0`). That is what made
"serve `None` until the first refresh lands" a non-event rather than an API break.
Check that property before adopting this pattern for a field that is currently
non-optional — the consumer set for a summaries-style payload is wide (sidebar
badge, carousel `diffStatsOverride`, kanban board, issue panel, project sidebar,
remote-web host list).

## Client-side: the list re-derives itself on every patch

Two compounding costs, both worth checking in any WS-fed list:

- **Sort keys derived inside the comparator.** `new Date(value).getTime()` and
  `localeCompare` ran per comparison, i.e. ~2·n·log n times per sort pass.
  Decorate–sort–undecorate drops that to n. Note the *pathological* case: while
  the summaries response has not arrived, every row's timestamp is absent, so
  **every** pair falls through to the `localeCompare` tiebreak.
- **Fresh row objects on every patch.** Mapping the stream into new view-model
  objects on each render means element identity always changes, which invalidates
  every downstream filter/sort memo and makes `React.memo` on the row useless.
  Memoise per id on `(record, summary)` reference identity and prune ids that
  leave the stream. `React.memo` on the row, stable per-row callbacks, and stable
  row objects only pay off **together** — shipping one without the others is a
  no-op.

This is the root cause of a symptom already recorded in
`wiki/workspace-carousel-view.md` ("the sort input changes identity frequently
with *equal content*"), which the carousel worked around with an `arraysEqual`
content compare. Fixing the source does not make that workaround removable — the
stream still patches with equal content for other reasons.

### Two traps found by testing rather than review

- **Deleting a "redundant" sort can break a hidden consumer.** `useWorkspaces`
  pre-sorted by pinned-then-`created_at`, and the sidebar re-sorted immediately
  after, so the hook's sort looked like pure waste — but `CreateModeProvider`
  read `activeWorkspaces[0]` to seed project selection. Grep for index access and
  head-element reads, not just for `.sort`, before removing an ordering guarantee.
- **Assert ordering equality against a copy of the old comparator**, not against
  a hand-written expectation. Doing so surfaced a real latent bug: the old
  comparator branched on `a.isPinned !== b.isPinned` without normalising, so
  `false` sorted ahead of `undefined`. Unreachable in production (`pinned` is a
  non-null boolean on the wire), but the equality test is what turned "my rewrite
  differs" into a known, documented divergence instead of a silent one.

## Cheap DB wins in the same read path

`Workspace::find_all_with_status` selected every workspace with two correlated
subqueries each, then filtered `archived` and applied `limit` in Rust — so the
active request evaluated subqueries for 374 rows it discarded, four times per page
load. Push both into SQL (`WHERE ($1 IS NULL OR w.archived = $1)`,
`LIMIT COALESCE($2, -1)`) and index for it. `workspaces.archived` had **no** index
despite being the filter for every query behind the sidebar; nor did
`coding_agent_turns.seen`, which is scanned on a table that grows one row per
agent turn.

Changing any `sqlx::query!` here means re-running `pnpm run prepare-db` and
committing `crates/db/.sqlx` — CI runs `prepare-db:check` and fails on a stale
offline cache.

Still unfixed and worth knowing about: `find_all_with_status` also performs a
lazy name backfill that runs an **unbounded** `SELECT` of every execution
process's `executor_action` JSON plus a **write**, per unnamed workspace, on a read
path — against a single-writer SQLite. It is a no-op for named workspaces, which
is why it survives.

## Contributed by

- `d49f-loading-workspac`
