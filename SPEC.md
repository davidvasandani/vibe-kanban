# Technical Spec: Stop Recomputing Workspace Diff Stats From Disk On Every Poll

Task id: `d49f-loading-workspac`

> Constraints distilled from the project knowledge base are in
> [`PRIOR_KNOWLEDGE.md`](PRIOR_KNOWLEDGE.md); the load-bearing ones are folded
> into the design sections below and cited where they apply.

## The question being answered

> "Is it loading the Workspaces from disk every single time? What makes it so
> slow to sort?"

**Yes — every 15 seconds, for every workspace, active and archived, uncached,
over NFS.** The sort itself is not the bottleneck; it is a symptom. Two
independent costs stack up:

1. **Server:** `POST /api/workspaces/summaries` walks the working tree of every
   workspace with `git` subprocesses to produce three integers per workspace
   (`files_changed`, `lines_added`, `lines_removed`). Nothing is cached.
2. **Client:** the sidebar rebuilds and re-sorts every row object on every
   stream patch and every summaries poll, parses date strings *inside* the sort
   comparator, and re-renders all ~133 unmemoised rows.

### Measured, on the live coordinator (2026-08-03)

| Request | Workspaces | Latency (2 runs) |
| --- | --- | --- |
| `POST /api/workspaces/summaries {"archived":false}` | 132 | 5.64 s, 5.78 s |
| `POST /api/workspaces/summaries {"archived":true}` | 374 | 1.92 s, 1.67 s |

Both are polled on a 15 s `refetchInterval` with `refetchOnMount: 'always'`
(`packages/web-core/src/shared/hooks/useWorkspaces.ts:170-193`), so the
coordinator spends roughly **7.5 s of git/NFS work every 15 s, per open tab,
forever** — a ~50% duty cycle on a shared NFS mount, whether or not anything
changed. Archived workspaces are the majority of that row count and their
worktrees are usually already deleted, so most of that work cannot even produce
a meaningful number.

### Where the server time goes

`get_workspace_summaries`
(`crates/server/src/routes/workspaces/workspace_summary.rs:116-135`) fans out one
`compute_diff_stats` future per workspace with an unbounded `join_all`. Per
workspace **per repo** (`crates/services/src/services/diff_stream.rs:43-92`):

- one DB query for the repo list — **N+1**, one per workspace (`diff_stream.rs:51`);
- `Repository::open` on the shared bare repo + a `merge_base` walk
  (`crates/git/src/lib.rs:709-726`);
- `Repository::open` on the worktree, whose `.git` is a *file* indirection back
  into the shared repositories tree — two NFS path resolutions
  (`crates/git/src/lib.rs:334`);
- **four forked `git` processes** (`crates/git/src/cli.rs:187-247`):
  `read-tree HEAD` into a temp index, `status --porcelain -z` (an `lstat()` of
  every tracked path in the worktree), `add -A` (re-hashes the content of every
  changed/untracked file), `diff --cached -M --name-status`;
- then, for **each** changed entry (`crates/git/src/lib.rs:435-550`): inflate the
  base blob, `std::fs::read` the entire new file off NFS (up to 2 MiB), and run a
  full `git2::Patch::from_buffers` Myers diff — **only to sum two integers**
  (`crates/git/src/lib.rs:524-527`).

Everything except those three integers is then dropped on the floor. The mount is
NFSv3 with `timeo=600,retrans=2`, so one slow server turns the unbounded
`join_all` into a fleet-wide stall.

**Measured breakdown for one real worktree on this NFS mount** (a `vibe-kanban`
checkout with 13 changed files, timed with raw `git` so the numbers are not
Rust-specific):

| Stage | Cold cache | Warm, 3 runs |
| --- | --- | --- |
| Temp-index staging (`read-tree` + `status --porcelain -z` + `add -A`) | 7.00 s | 1.52 s / 1.65 s / 1.48 s |
| Final `diff --cached -M --numstat -z` | — | 0.044 s / 0.057 s / 0.055 s |

That reorders the priorities, and it is worth being explicit about because it is
the opposite of what the code's shape suggests: **the staging sweep is the cost,
and the final diff command is free.** So the cache (B2) is the change that
matters — it takes the request path from N × ~1.5 s to zero. Switching to
`--numstat` (B1) removes the per-changed-file NFS reads and Myers diffs stacked on
*top* of that staging cost, which is a real but secondary win. Any future attempt
to make this cheaper still has to attack the staging sweep (`core.fsmonitor`, an
untracked cache, or `add -N` to avoid hashing), not the diff.

### Why the list feels slow to sort

- `getWorkspaceSortTimestamp` → `toTimestamp` calls `new Date(value).getTime()`
  **inside the comparator**
  (`packages/web-core/src/pages/workspaces/WorkspacesSidebarContainer.tsx:235-253,489-490`)
  — ~2·n·log n date-string parses per sort pass, plus `localeCompare` on every
  tie (and *every* pair ties while `latestProcessCompletedAt` is still
  undefined, i.e. until the first summaries response lands).
- A second, redundant sort with the same `new Date()`-in-comparator pattern runs
  first in `useWorkspaces.ts:195-225`, and its result is thrown away and
  re-sorted by the container.
- That same memo re-allocates a brand-new `SidebarWorkspace` object for **every**
  row on every stream patch and every 15 s poll
  (`useWorkspaces.ts:61-91,208,224`), so element identity always changes → every
  downstream filter/sort memo is invalidated → all rows re-render.
- Rows are not memoised (`packages/ui/src/components/WorkspaceSummary.tsx:59`)
  and each gets a fresh `onClick` closure
  (`packages/ui/src/components/WorkspacesSidebar.tsx:490`), so a re-render of the
  container is a re-render of all rows.
- The 50-row page limit is **disabled while searching**
  (`WorkspacesSidebarContainer.tsx:525-531`) and there is no debounce on the
  search input, so every keystroke filters + sorts + renders the full matching
  set.

## Goals

- **G1.** Sustained server cost of the sidebar becomes **DB-only**: no `git`
  subprocess, no worktree stat sweep, no file read on the request path.
- **G2.** Diff-stat freshness stays good enough to be useful: stale by at most one
  refresh interval in the common case, and promptly correct after anything the
  server knows about changes the worktree.
- **G3.** Total git work is *bounded* — capped concurrency, no work for
  workspaces that cannot have a diff, and independent of how many browser tabs
  are open.
- **G4.** Sorting the list is O(n) key extraction plus one comparison pass over
  precomputed primitives, with rows that only re-render when their own data
  changes.
- **G5.** No change to the public API shape and no visible change to what the
  sidebar displays, beyond it appearing sooner.

## Non-goals

- Virtualising the sidebar list. Worth doing, but the 50-row page limit plus
  memoised rows removes the pain at 133 rows; tracked as a follow-up.
- Splitting the monolithic `WorkspaceContext` value. Real, but a much wider
  refactor.
- Changing the `useBranchStatus` 5 s poll (which does a `git fetch` per repo).
  That is open-workspace cost, not list cost.
- Making the archived list lazy (fetching it only when the archive is shown).
  That changes what backfills after archiving; follow-up.

## Design

### B1. Compute diff stats with `--numstat` instead of materialising diffs

`git diff --cached -M --numstat` returns exactly the three numbers the sidebar
wants, from a subprocess the code **already runs**. Add a numstat path alongside
the existing `--name-status` path:

- `crates/git/src/cli.rs`: extract the temp-index preparation currently inlined
  in `diff_status` (`:193-233`) into a private helper, and add
  `GitCli::diff_numstat(worktree_path, base_commit) -> Result<DiffNumstat, GitCliError>`
  that reuses it and finishes with
  `git -c core.quotepath=false diff --cached -M --numstat -z <base>`. `-z` keeps
  path handling NUL-safe and consistent with `get_worktree_status`; binary files
  report `-\t-` and contribute 0 added / 0 removed.
- `crates/git/src/lib.rs`: add
  `GitService::get_diff_stats(worktree_path, base_commit) -> Result<DiffStats, GitServiceError>`
  wrapping it. It needs **no** `Repository::open` of the worktree and no base
  tree at all.
- `crates/services/src/services/diff_stream.rs`: `compute_diff_stats` calls
  `get_diff_stats` instead of `get_diffs`.

**The temp index is load-bearing and must be kept.** A bare
`git diff --numstat <base>` counts neither staged nor untracked content — that is
the `include_untracked(false)` trap recorded in
`docs/knowledge-base/workspace-directory-reclamation.md` ("**misses untracked
files entirely**"). The switch is behaviour-preserving *only* because it keeps
`read-tree HEAD` → `status --porcelain -z` → `add -A` under `GIT_INDEX_FILE` and
changes nothing but the final command. Do not "simplify" the temp index away.

Note also that `git add -A` writes loose objects into the **shared** object store
even though the index itself is a `/tmp` file. That is pre-existing behaviour;
this task reduces its frequency rather than removing it. Removing it
(`git add -N` plus a non-`--cached` diff) is a follow-up, not a drive-by.

Semantics: `files_changed` is the same entry count as today. Line counts become
*more* accurate — today a file over `MAX_INLINE_DIFF_BYTES` (2 MiB) or one
tripping a UTF-8/binary guard silently contributes `0/0`
(`crates/git/src/lib.rs:394-431`), whereas `--numstat` counts real text lines and
reports `-` only for genuinely binary blobs. This removes one `Repository::open`,
all blob inflation, all `std::fs::read` of worktree files, and all Myers diffs
from the path. The remaining per-repo cost is the merge-base walk plus the four
`git` processes.

### B2. Cache the stats, serve stale, refresh in the background

New service `crates/services/src/services/workspace_diff_stats.rs`, modelled on
`FileSearchCache` (`crates/services/src/services/file_search.rs:86`) and exposed
the same way — an `Arc<WorkspaceDiffStatsCache>` on `LocalDeployment` with a
`workspace_diff_stats(&self)` accessor on the `Deployment` trait
(`crates/deployment/src/lib.rs:107` is the precedent; `LocalDeployment` is the
only implementor).

```rust
pub struct WorkspaceDiffStatsCache {
    entries: moka::future::Cache<Uuid, CachedDiffStats>, // long TTL, bounded capacity
    inflight: Mutex<HashSet<Uuid>>,                     // dedupe concurrent refreshes
    permits: Arc<Semaphore>,                            // bound NFS fan-out
}

struct CachedDiffStats { stats: DiffStats, computed_at: Instant }
```

Public surface:

- `snapshot(ids) -> HashMap<Uuid, DiffStats>` — pure in-memory read, no I/O.
- `refresh_stale(pool, git, workspaces)` — spawns one background task that
  computes entries that are missing or older than `REFRESH_AFTER`, skipping ids
  already inflight, each acquiring a permit before touching disk.
- `invalidate(id)` / `invalidate_many(ids)` — drop entries so the next refresh
  recomputes them.

The request path becomes:

```rust
let stats = deployment.workspace_diff_stats().snapshot(&ids);            // in-memory
deployment.workspace_diff_stats().refresh_stale(pool, git, &workspaces); // fire and forget
```

The handler no longer awaits any git work, so its latency is the DB queries
alone. This is **stale-while-revalidate**: a poll serves what the previous
refresh produced and schedules the next one. Because the client already polls
every 15 s, a value computed during poll *k* is displayed at poll *k+1*.

Constants: `REFRESH_AFTER = 60s`, `MAX_CONCURRENT_REFRESH = 8`, entry TTL 1 h,
capacity 2048 (TTL **and** a hard cap — the established idiom, per
`docs/knowledge-base/mcp-oauth-connect.md`; unbounded in-memory growth has caused
an OOM in this repo before). `REFRESH_AFTER` at 60 s (rather than 15 s) means the
git work runs at most once a minute per workspace instead of four times, and the
semaphore means at most 8 worktrees are walked at any instant instead of 132.

Three invariants that are not negotiable:

- **A failure is not a zero.** `compute_diff_stats` today `continue`s past a
  failed repo and returns a zeroed struct, which is why cleaned-up workspaces
  report a confident "0 files changed". The cache must store an entry **only**
  when every repo computed successfully; otherwise leave it absent so the field
  stays `None`. "A git probe that errors is not a clean repo", and an unreachable
  worktree is indeterminate, not idle
  (`docs/knowledge-base/workspace-directory-reclamation.md`,
  `clustered-workspace-execution.md`).
- **A stale refresh must not clobber a fresher value.** A refresh that started
  before an `invalidate` must not write its result afterwards. Carry a
  monotonic per-workspace generation, captured before the git work and compared
  before the insert (the pattern in `docs/knowledge-base/active-mcp-refresh.md`).
  Readers must never see a torn `DiffStats`, so the whole struct is inserted
  atomically.
- **`inflight` is cleared unconditionally**, via a guard, not on the success
  path — otherwise a workspace whose refresh keeps failing is never retried
  ("retain-on-error means retain-forever").

**No `tokio::time::timeout` around the git work.** The repo's usual bounded
fan-out pairs `join_all` with per-probe timeouts, but this work runs inside
`spawn_blocking`, which a timeout cannot cancel — it would release the semaphore
permit while the thread stayed stuck on NFS, which is worse than waiting. The
permit is held for the whole blocking call, and that plus `inflight` dedupe is
the bound. Recorded as a deliberate deviation.

Cold start: the first response after server boot has `None` stats and badges
appear on the next poll (≤15 s). That is strictly better than today, where the
rows already render immediately from the WebSocket stream and the badges only
arrive when the 5.7 s request finishes. `files_changed` and friends are already
`Option<usize>` (`workspace_summary.rs:34-38`), and **every** consumer either
`?? 0`s them or gates on `!== undefined` — the sidebar badge is
`hasChanges = filesChanged !== undefined && filesChanged > 0`
(`packages/ui/src/components/WorkspaceSummary.tsx:81`), and the other readers are
`WorkspaceCarouselColumn.tsx:158`, `KanbanContainer.tsx:683`,
`IssueWorkspacesSectionContainer.tsx:95`, `ProjectRightSidebarContainer.tsx:407`
and `packages/remote-web/src/routes/hosts.$hostId.workspaces.tsx:109`. So `None`
renders identically to `0` and needs no type or UI change (G5).

### B3. Invalidate on the events the server already observes

TTL alone would make an agent's edits show up up to 60 s late. The server already
knows when a workspace's tree plausibly changed, and there is an established
mechanism for exactly this: subscribe to the SQLite-hook JSON-patch stream rather
than threading the cache through container finalization. Model the watcher on
`BrowserSessionService::spawn_cleanup_watcher`
(`crates/services/src/services/browser/mod.rs:622-682`) — take `Arc<MsgStore>`,
`Arc::downgrade` the inner state so dropping the service stops the task instead of
leaking it, `tokio::spawn`, and match on patch paths.

- **`/workspaces/{uuid}` Add/Replace → invalidate that workspace.** This one hook
  covers the whole execution lifecycle for free: `push_workspace_update_for_session`
  (`crates/services/src/services/events.rs:43-55,270-284`) already pushes a
  workspace replace patch on **every** `execution_processes` insert/update, so
  agent runs, setup/cleanup scripts and dev servers are all captured at both start
  and finish, with no session→workspace lookup. Spurious invalidations (a `touch`,
  a rename) cost at most one extra recompute on the next sweep.
- **Git mutations that change the diff but not the `workspaces` row.** Of the
  handlers in `crates/server/src/routes/workspaces/git.rs:139-148`, the ones that
  need an explicit invalidation are `change_target_branch` (writes
  `workspace_repos`, which is **not** a hooked table — and it moves the merge
  base, so the cached number is not merely stale but wrong), `rebase_workspace`,
  `continue_workspace_rebase`, `abort_workspace_conflicts` and `merge_workspace`.
  `rename_branch` is covered for free because it updates `workspaces.branch`;
  `merge_workspace` gets an explicit call anyway because a **pinned** workspace is
  not archived afterwards, so it would not otherwise emit a patch.
  `push`/`force push` are deliberately **not** invalidated — publishing a branch
  to the remote does not change the diff against the merge base.

**The broadcast is lossy, and `REFRESH_AFTER` is what makes this safe.** The
watcher must handle `RecvError::Lagged(_)` by continuing (dropped events are
skipped), exactly as the browser watcher does; "broadcast lag can drop events —
the lease TTL and idle sweep are the backstop, which is what makes this design
safe" (`wiki/browser-session-control-arbiter.md`). So invalidation is a *latency*
optimisation layered on a level-triggered sweep, never the mechanism of record —
the same discipline as `docs/knowledge-base/issue-status-side-effects.md`.
`REFRESH_AFTER` must not be made conditional on having seen an event.

Not covered: edits made directly on disk with no VK process running (an external
editor, or a shell outside a VK terminal session). Those are bounded by
`REFRESH_AFTER`. Explicitly accepted — the sidebar badge is a soft signal, and
opening the workspace shows the authoritative live diff.

Note that nothing self-drives the sweep: `refresh_stale` is called by the
summaries request, so with no client connected nothing refreshes. That is
intended (no idle cost), but tests must not assume a background timer exists.

### B4. Do not do work that cannot produce an answer

`workspace_summary.rs:122` gates only on `container_ref.is_some()`. But
`cleanup_workspace` marks `worktree_deleted = true` **without** clearing
`container_ref` (`crates/local-deployment/src/container.rs:922`), so every
cleaned-up workspace still pays for a bare-repo open and a merge-base walk each
cycle and then reports a misleading `0/0/0` instead of "unknown".

Skip refresh when `worktree_deleted` is true, and report `None` rather than a
cached zero for those workspaces. For the archived list — 374 rows whose
worktrees are almost all deleted — this removes essentially the entire 1.8 s.

`worktree_deleted` and `container_ref` are **advisory, not authoritative**: a
repo-wide `git worktree prune` during another workspace's cleanup, or OS reaping
of the `/var/tmp` base dir, can remove a working tree while the DB columns survive
(`docs/knowledge-base/workspace-directory-reclamation.md`). So they are a cheap
skip filter, never proof that a cached number is still valid — which is the other
reason the TTL stays.

The N+1 `WorkspaceRepo` query at `diff_stream.rs:51` is **deliberately left
alone.** It only mattered because it ran 132× every 15s alongside the git work;
once refreshes are capped at once per 60s per workspace and gated by a semaphore,
it is an indexed SQLite point lookup (`idx_workspace_repos_workspace_id`) on a
background task. Batching it would mean a runtime-checked `IN (…)` query and a
hand-written `FromRow` for the flattened `RepoWithTargetBranch` — new machinery
with no measurable payoff. Recorded as a follow-up instead.

### B5. Make the list query do its own filtering

`Workspace::find_all_with_status` (`crates/db/src/models/workspace.rs:783-874`)
selects **every** workspace with two correlated subqueries each, then filters
`archived` (`:855`) and applies `limit` (`:859-861`) in Rust. With ~506 rows the
`archived=false` request evaluates those subqueries for 374 rows it discards, and
this runs four times per page load (two streams + two summaries) plus twice per
15 s.

- Push the `archived` predicate and `LIMIT` into SQL, keeping the
  `Option<bool>` / `Option<i64>` signature (`None` = no filter) via
  `COALESCE`-style binds so one cached SQLx query still serves every caller.
- Add migration `crates/db/migrations/<ts>_index_workspace_list_paths.sql`:
  - `CREATE INDEX IF NOT EXISTS idx_workspaces_archived_updated_at ON workspaces(archived, updated_at DESC);`
    — covers this `WHERE`+`ORDER BY` and the `WHERE w.archived = ?` in
    `find_latest_for_workspaces`, `find_workspaces_with_running_dev_servers`,
    `get_latest_for_workspaces` and `find_workspaces_with_unseen`, none of which
    has an index on `archived` today.
  - `CREATE INDEX IF NOT EXISTS idx_coding_agent_turns_unseen ON coding_agent_turns(seen) WHERE seen = 0;`
    — `find_workspaces_with_unseen` full-scans a table that grows one row per
    agent turn.
- Re-run `pnpm run prepare-db` and commit `crates/db/.sqlx`. This is enforced:
  `.github/workflows/test.yml:147` runs `npm run prepare-db:check`, so a stale
  offline cache fails CI.

The unbounded `get_first_user_message` name backfill (a `SELECT` of every
execution process's `executor_action` JSON, plus a **write**, per unnamed
workspace, on a read path — `workspace.rs:863-871`) is deliberately left alone:
it is guarded by `name IS NULL` so it is a no-op for named workspaces, and
bounding it correctly needs its own change. Recorded as a follow-up.

### F1. Precompute sort keys; drop the redundant sort

In `WorkspacesSidebarContainer.tsx`, replace comparator-time date parsing with
decorate–sort–undecorate: one `useMemo` maps the filtered list to
`{ ws, pinned, ts, nameKey }` (`ts` from a single `Date.parse`, `nameKey` from
`ws.name.toLowerCase()`), sorts on those primitives, then maps back. That turns
~1 900 date parses and every `localeCompare` per sort pass into 133 of each.
Ordering semantics — pinned first, null timestamps first, name as tiebreak,
asc/desc — are preserved exactly.

Delete the first sort in `useWorkspaces.ts:195-225`: the container re-sorts by
the user's preference immediately afterwards, so it is pure waste.

One consumer did depend on that ordering — `CreateModeProvider` reads
`activeWorkspaces[0] ?? archivedWorkspaces[0]` to seed project selection. It gets
its own head-element pick, preserving the old precedence exactly (pinned before
unpinned, then newest `createdAt`, active before archived). Every other consumer
of `activeWorkspaces` builds a `Map` or calls `.find()`, and the carousel applies
its own sort, so they are order-independent — verified by grep, not assumed.

### F2. Keep row identity stable across patches

`toSidebarWorkspace` allocates a new object per row on every patch and every
poll. Memoise per workspace id on `(WorkspaceWithStatus, WorkspaceSummary)`
identity so unchanged rows keep the same object reference. Combined with F3 this
turns "any patch re-renders 133 rows" into "a patch re-renders the rows it
touched".

This is the *cause* of a problem the knowledge base already records —
"Workspace summaries repoll every 15s and the WS stream patches often, so the
sort input changes identity frequently with *equal content*"
(`wiki/workspace-carousel-view.md`). The carousel worked around it with an
`arraysEqual` content compare. **Leave that workaround in place**: the WS stream
still patches with equal content for other reasons, so removing it here would be
an unrelated regression risk.

### F3. Memoise the row component

Wrap `WorkspaceSummary` (`packages/ui/src/components/WorkspaceSummary.tsx:59`) in
`React.memo`, and stop creating a fresh `onClick` closure per row in
`WorkspacesSidebar.tsx` (`:171,373,490`) — pass the workspace id back through a
stable handler. `React.memo` is inert without F2 and F2 is wasted without
`React.memo`; they ship together.

### F4. Bound the search path

- Debounce the search query by 150 ms before it feeds the filter memos, keeping
  the input itself responsive. **The armed timer must survive unrelated
  re-renders** and read its target from a ref — an effect that clears and re-arms
  in its cleanup resets the countdown on every unrelated update and can starve
  indefinitely, which is a trap this repo has already hit
  (`wiki/workspace-carousel-view.md`).
- Apply `displayLimit` while searching too (`:525-539`). "Show the top 50
  matches, load more on scroll" is the same contract as the unsearched list, and
  it stops a two-character query from rendering every row.

## Testing

- `crates/git`: unit tests for numstat parsing (added / modified / deleted /
  renamed, binary `-\t-`, paths with spaces and non-UTF-8 bytes) and a temp-repo
  test asserting `get_diff_stats` agrees with `get_diffs`' aggregate on a fixture
  with a text change, a rename, an untracked file and a binary file.
- `crates/services`: `WorkspaceDiffStatsCache` — a miss returns `None` and
  enqueues; a second `refresh_stale` while one is inflight does not double-run;
  entries younger than `REFRESH_AFTER` are not recomputed; `invalidate` forces
  recomputation; the semaphore caps concurrency; `worktree_deleted` workspaces
  are never scheduled.
- `crates/db`: `find_all_with_status` returns the same rows and order for
  `Some(true)`, `Some(false)` and `None`, and honours `limit`.
- Frontend (Vitest, colocated): the new sort helper matches the current
  comparator's ordering across a fixture covering pinned rows, missing
  timestamps, equal timestamps and both sort orders; and the memoised row
  transform returns identical references for unchanged input.
- Manual, against the live coordinator: `POST /workspaces/summaries` latency for
  `archived=false` drops from ~5.7 s to DB-only on the second and subsequent
  calls, and badge values match the pre-change response. The measurements already
  in this spec are the required control run on the unfixed code — a latency
  number that also looks good before the fix proves nothing
  (`docs/knowledge-base/workspace-directory-reclamation.md`). Verification stays
  **read-only HTTP**: do not start a dev server against the shared host, because
  `cleanup_orphan_workspaces` always sweeps the default base dir and would
  classify live worktrees there as orphans.
- `pnpm install --frozen-lockfile` first (fresh worktree), then
  `pnpm run check`, `pnpm run lint`, `cargo test --workspace`,
  `pnpm run generate-types:check`, `pnpm run prepare-db`, `pnpm run format`.
- No CI change-filter edits needed: every path touched is already covered by
  `.github/workflows/test.yml:41-92` (checked, not assumed).

## Risks

- **Stale badges.** Mitigated by B3 invalidation and a 60 s ceiling; the open
  workspace still shows a live diff. Worst realistic case: a file edited outside
  VK shows a 60 s-old count in the sidebar.
- **`--numstat` disagreeing with today's numbers.** It will disagree — by being
  right — for >2 MiB and non-UTF-8-guarded files that currently report `0/0`.
  Covered by a cross-check test; called out as intended.
- **Cold-start blank badges** for one poll interval. No worse than the current
  5.7 s wait.
- **Pushing `archived`/`LIMIT` into SQL** must not change ordering or the `None`
  case; covered by same-result DB tests.
- **`React.memo` masking updates** if a prop were mutated rather than replaced.
  F2 replaces objects wholesale, and the memo test asserts reference behaviour.

## Follow-ups (out of scope, worth filing)

1. Virtualise the sidebar list (`@tanstack/react-virtual` is already a
   `packages/ui` dependency and used in `ChangesPanel.tsx`).
2. Split `WorkspaceContext` so a session/repo change does not re-render the list.
3. Bound `Workspace::get_first_user_message` and move the name backfill off the
   read path.
4. Fetch archived summaries only when the archive section is open.
5. `useWorkspaces` is called from three places, each opening its own pair of
   WebSockets (`CreateModeProvider`, `RebaseDialog`); share one subscription.
6. `useBranchStatus`' 5 s poll does a `git fetch` per repo for the open
   workspace.
7. Attack the temp-index staging sweep itself — it is ~1.5 s warm / 7 s cold per
   worktree and dwarfs everything else. Options: `core.fsmonitor`, an untracked
   cache, or `git add -N` plus a non-`--cached` diff (which would also stop
   writing loose objects into the shared object store).
8. Batch `WorkspaceRepo::find_repos_with_target_branch_for_workspace`, now that
   it is the only remaining per-workspace query in the refresh loop.
