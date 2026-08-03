# Implementation Plan: Workspace List Load/Sort Cost (`d49f-loading-workspac`)

Companion to [`SPEC.md`](SPEC.md) and [`PRIOR_KNOWLEDGE.md`](PRIOR_KNOWLEDGE.md).

Ordered so each step compiles, tests, and is independently reviewable. Steps 1–2
are pure speedup with no behaviour change; step 3 is the architectural change;
steps 4–6 are cleanups; steps 7–10 are frontend. Backend and frontend do not
depend on each other and could ship separately.

Prerequisite (already done in this worktree): `pnpm install --frozen-lockfile`.

---

## Step 1 — `git diff --numstat` path in `crates/git`

**Files:** `crates/git/src/cli.rs`, `crates/git/src/lib.rs`

1. In `cli.rs`, extract the temp-index preparation currently inlined in
   `diff_status` (`:193-233` — `TempDir`, `GIT_INDEX_FILE` env, `read-tree HEAD`,
   `get_worktree_status`, `add -A --pathspec-from-file=- --pathspec-file-nul`)
   into a private helper:

   ```rust
   fn stage_worktree_into_temp_index(
       &self,
       worktree_path: &Path,
   ) -> Result<(tempfile::TempDir, Vec<(OsString, OsString)>), GitCliError>
   ```

   Return the `TempDir` so the caller keeps it alive — dropping it deletes the
   index. Rewrite `diff_status` to call it, changing nothing else about that
   function's behaviour.
2. Add `pub struct DiffNumstat { files_changed: usize, lines_added: usize, lines_removed: usize }`
   and `GitCli::diff_numstat(&self, worktree_path, base_commit) -> Result<DiffNumstat, GitCliError>`:
   call the helper, then run
   `git -c core.quotepath=false diff --cached -M --numstat -z <base>` under the
   same envs, and parse.
3. Parsing `--numstat -z`: records are `added\tremoved\t` followed by the path.
   For renames/copies `-z` emits `added\tremoved\t\0old\0new\0`; for everything
   else `added\tremoved\tpath\0`. Binary files report `-` for both counts. Count
   one changed file per record; add `-` as 0. Put the parser in a free function
   `parse_numstat_z(&[u8]) -> DiffNumstat` so it is unit-testable without a repo.
4. In `lib.rs`, add:

   ```rust
   pub fn get_diff_stats(
       &self,
       worktree_path: &Path,
       base_commit: &Commit,
   ) -> Result<DiffStats, GitServiceError>
   ```

   next to `get_diff_file_paths` (`:358`), delegating to `diff_numstat`. Note it
   needs **no** `Repository::open` and no base tree — that is the point. Re-export
   whatever type it returns from `crates/git`.

**Do not** drop the temp index in favour of a bare `git diff --numstat <base>`:
that stops counting staged and untracked content (see `SPEC.md` B1 and
`PRIOR_KNOWLEDGE.md` constraint 14).

**Tests (`crates/git`):**
- `parse_numstat_z` unit tests: added, modified, deleted, rename (3-field form),
  binary `-\t-`, path with a space, path with non-UTF-8 bytes, empty input,
  trailing NUL.
- Temp-repo integration test: build a fixture with a modified tracked file, a
  rename, an untracked file and a binary file; assert `get_diff_stats` equals the
  aggregate of `get_diffs` for `files_changed`, and equals it for line counts on
  the text cases. Assert explicitly that the untracked file **is** counted.

**Verify:** `cargo test -p git`.

---

## Step 2 — Use it, and batch the per-workspace repo query

**Files:** `crates/services/src/services/diff_stream.rs`,
`crates/db/src/models/workspace_repo.rs`

1. `compute_diff_stats` (`diff_stream.rs:43-92`): replace the second
   `spawn_blocking` (`get_diffs` + the summing loop) with `get_diff_stats`.
2. **Change the error contract.** Today a failed repo `continue`s and the function
   returns `Some(zeroed)`. Make it return `None` if *any* repo fails, so callers
   can distinguish "no changes" from "could not compute". Introduce a small
   internal result type or just `Option`; keep the signature
   `-> Option<DiffStats>`.
   - Audit callers of `compute_diff_stats` before changing this. As of now the
     only ones are `workspace_summary.rs:176` and the live diff stream. Confirm
     with `rg 'compute_diff_stats'`.
3. Add `WorkspaceRepo::find_repos_with_target_branch_for_workspaces(pool, &[Uuid]) -> HashMap<Uuid, Vec<RepoWithTargetBranch>>`
   next to the existing single-workspace version (`workspace_repo.rs:137`). SQLite
   has no array binding, so build the `IN (?, ?, …)` list with
   `sqlx::QueryBuilder` (runtime-checked, so no `.sqlx` entry needed) and chunk at
   a few hundred ids to stay under `SQLITE_MAX_VARIABLE_NUMBER`.
4. Add a variant of `compute_diff_stats` that takes the already-resolved
   `Vec<RepoWithTargetBranch>` so the batch refresher (step 3) does one query
   total. Keep the existing per-workspace function as a thin wrapper so the live
   diff stream is untouched.

**Tests:** `crates/db` test that the batched query returns the same rows as N
calls to the single-workspace version, including the empty-id-list case.

**Verify:** `cargo test -p db -p services`, then `pnpm run prepare-db` (step 3
adds SQL too; run it once at the end of step 6).

---

## Step 3 — `WorkspaceDiffStatsCache`

**New file:** `crates/services/src/services/workspace_diff_stats.rs`
**Touched:** `crates/services/src/services/mod.rs`, `crates/deployment/src/lib.rs`,
`crates/local-deployment/src/lib.rs`,
`crates/server/src/routes/workspaces/workspace_summary.rs`

### 3a. The cache

```rust
pub const REFRESH_AFTER: Duration = Duration::from_secs(60);
const MAX_CONCURRENT_REFRESH: usize = 8;
const CACHE_CAPACITY: u64 = 2048;
const ENTRY_TTL: Duration = Duration::from_secs(3600);

struct Inner {
    entries: moka::future::Cache<Uuid, CachedDiffStats>,
    generations: DashMap<Uuid, u64>,   // bumped by invalidate()
    inflight: DashMap<Uuid, ()>,       // dedupe
    permits: Semaphore,
}

#[derive(Clone)]
struct CachedDiffStats { stats: DiffStats, computed_at: Instant }

pub struct WorkspaceDiffStatsCache { inner: Arc<Inner> }
```

`moka 0.12` with the `future` feature and `dashmap 6.1` are already dependencies
of `crates/services`. Build the moka cache with `.max_capacity(CACHE_CAPACITY)`
and `.time_to_live(ENTRY_TTL)` — TTL **and** a hard cap, per
`PRIOR_KNOWLEDGE.md` constraint 16.

Public API:

- `pub fn snapshot(&self, ids: &[Uuid]) -> HashMap<Uuid, DiffStats>` — in-memory
  only, no `.await` on I/O.
- `pub fn refresh_stale(&self, pool: &SqlitePool, git: &GitService, workspaces: &[Workspace])`
  — synchronous, returns immediately, `tokio::spawn`s the work. Selects targets
  with a **pure helper** (constraint 25):

  ```rust
  pub fn needs_refresh(ws: &Workspace, cached: Option<Instant>, now: Instant) -> bool
  ```

  `false` when `ws.container_ref.is_none()`, when `ws.worktree_deleted`, or when
  `cached` is younger than `REFRESH_AFTER`; otherwise `true`. Unit-test as a truth
  table.
- `pub fn invalidate(&self, id: Uuid)` / `invalidate_many(&self, ids: &[Uuid])` —
  bump the generation and remove the entry.
- `pub fn spawn_invalidation_watcher(&self, msg_store: Arc<MsgStore>)` — see 3c.

Refresh task rules — each is a review checkpoint:

1. Resolve every target's repos in **one** batched query (step 2).
2. For each target: `inflight.insert` returning early if already present; acquire
   a semaphore permit; capture `generation` **before** the git work; compute; then
   insert **only if** the generation is unchanged *and* the computation returned
   `Some`. A failed computation stores nothing (constraint 11 — a failure is not a
   zero).
3. Clear the `inflight` marker from a drop guard, not on the success path, so a
   persistently failing workspace is retried (constraint 18).
4. Hold the permit across the whole `spawn_blocking`. **No `tokio::time::timeout`**
   — `spawn_blocking` is not cancellable and a timeout would free the permit while
   the thread stayed stuck on NFS (`SPEC.md` B2, constraint 19). Document this in
   a comment; it will otherwise read as an oversight in review.

### 3b. Wiring

- `crates/deployment/src/lib.rs`: add
  `fn workspace_diff_stats(&self) -> &Arc<WorkspaceDiffStatsCache>;` to the
  `Deployment` trait, directly modelled on `file_search_cache` (`:107`).
  `LocalDeployment` is the only implementor.
- `crates/local-deployment/src/lib.rs`: construct it next to
  `FileSearchCache::new()` (`:343`), store the field (`:69`, `:382`), implement
  the accessor (`:452`), and call `spawn_invalidation_watcher` right where
  `browser_sessions.spawn_cleanup_watcher(events.msg_store().clone())` is called
  (`:341`).
- `workspace_summary.rs`: delete the `join_all` block (`:116-135`) and
  `compute_workspace_diff_stats` (`:172-188`); replace with a `snapshot` read plus
  a `refresh_stale` call. Assembly (`:138-164`) is unchanged — `stats.map(...)`
  already yields `None` on a miss.

### 3c. Invalidation watcher

Copy the shape of `BrowserSessionService::spawn_cleanup_watcher`
(`crates/services/src/services/browser/mod.rs:622-682`) exactly:

- `let weak = Arc::downgrade(&self.inner);` then `tokio::spawn`.
- `msg_store.get_receiver()`; `RecvError::Lagged(_) => continue`,
  `Closed => break`; `let Some(inner) = weak.upgrade() else { break };`
  (constraints 5, 6).
- Match `LogMsg::JsonPatch`, and for each op whose path starts
  `/workspaces/{uuid}` on `Add`/`Replace`, `invalidate(uuid)`. That single hook
  covers all execution-process transitions, because
  `push_workspace_update_for_session` already pushes a workspace patch on every
  `execution_processes` insert/update (`events.rs:43-55,270-284`).

**Tests (`crates/services`):**
- `needs_refresh` truth table: no `container_ref`; `worktree_deleted`; fresh
  entry; stale entry; missing entry.
- `snapshot` on an empty cache returns nothing and does no I/O.
- Second `refresh_stale` while one is inflight does not double-compute (assert a
  call counter via an injected compute closure — keep the compute step behind a
  small trait or `Fn` so tests need no git repo).
- A refresh whose generation was bumped mid-flight does not insert.
- A failing computation leaves the entry absent and clears `inflight`.
- Concurrency never exceeds `MAX_CONCURRENT_REFRESH` (max-observed counter).
- Watcher: a `/workspaces/{id}` replace patch invalidates; an unrelated path does
  not; `Lagged` does not kill the task.

**Verify:** `cargo test -p services -p server`, plus `cargo build --workspace`
for the trait change.

---

## Step 4 — Push `archived` and `LIMIT` into SQL

**File:** `crates/db/src/models/workspace.rs`

Rewrite `find_all_with_status` (`:783-874`):

```sql
FROM workspaces w
WHERE ($1 IS NULL OR w.archived = $1)
ORDER BY w.updated_at DESC
LIMIT COALESCE($2, -1)
```

(`LIMIT -1` is SQLite's "no limit".) Bind `Option<bool>` and `Option<i64>`
directly so one cached `query!` still serves every caller. Delete the Rust-side
`.filter` (`:855`) and `truncate` (`:859-861`). Leave the name-backfill loop
(`:863-871`) alone — deliberate, recorded as follow-up 3 in `SPEC.md`.

**Tests (`crates/db`):** seed archived and active workspaces; assert
`Some(true)`, `Some(false)` and `None` return the same sets and the same
`updated_at DESC` order as the previous implementation, and that `limit` truncates
the same way.

---

## Step 5 — Indexes

**New file:** `crates/db/migrations/<timestamp>_index_workspace_list_paths.sql`

```sql
CREATE INDEX IF NOT EXISTS idx_workspaces_archived_updated_at
    ON workspaces (archived, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_coding_agent_turns_unseen
    ON coding_agent_turns (seen) WHERE seen = 0;
```

Match the existing migration naming in `crates/db/migrations/`. Column order must
match the pushed-down `WHERE archived = ? ORDER BY updated_at DESC` exactly
(constraint: "the lookup, the stored path, and the index expression must match").

---

## Step 6 — Invalidate on git mutations, then regenerate the SQLx cache

**File:** `crates/server/src/routes/workspaces/git.rs`

After each of `merge_workspace`, `push_workspace_branch`,
`force_push_workspace_branch`, `rebase_workspace`, `continue_workspace_rebase`,
`abort_workspace_conflicts` and `change_target_branch` succeeds, call
`deployment.workspace_diff_stats().invalidate(workspace_id)`. `change_target_branch`
is the important one: it moves the merge base, so a cached value is wrong, not
merely stale.

Then, once all SQL changes are in:

```
pnpm run prepare-db          # regenerates crates/db/.sqlx — must be committed
pnpm run prepare-db:check    # what CI runs (test.yml:147)
```

---

## Step 7 — Precompute sort keys; delete the redundant sort

**Files:** `packages/web-core/src/pages/workspaces/WorkspacesSidebarContainer.tsx`,
`packages/web-core/src/shared/hooks/useWorkspaces.ts`

1. New module `packages/web-core/src/pages/workspaces/workspaceSidebarSort.ts`
   (pure, unit-tested — mirrors `carousel/carouselSort.ts`):

   ```ts
   export interface SortKey { pinned: boolean; ts: number | null; nameKey: string }
   export function toSortKey(ws: Workspace, sortBy: WorkspaceSortBy): SortKey
   export function compareSortKeys(a: SortKey, b: SortKey, order: WorkspaceSortOrder): number
   export function sortWorkspaces(list: Workspace[], sortBy, sortOrder): Workspace[]
   ```

   `ts` uses `Date.parse` once per row (`NaN` → `null`). Ordering must be
   identical to the current comparator (`:481-512`): pinned first, `null`
   timestamps first, `nameKey` as the tiebreak both when both are null and when
   they are equal, `asc`/`desc` on the timestamp only.
2. Replace the `sortWorkspaces` `useCallback` (`:481-512`) with a call into the
   new module; delete `toTimestamp` and `getWorkspaceSortTimestamp` (`:235-253`).
3. Delete both `.sort(...)` blocks in `useWorkspaces.ts` (`:198-207`, `:214-223`),
   keeping the `.map(toSidebarWorkspace)`. The container re-sorts immediately
   afterwards.
   - Check first that no other consumer of `useWorkspaces().workspaces` relies on
     the created-at ordering: `rg 'useWorkspaces\(\)'` — currently
     `WorkspaceProvider`, `CreateModeProvider`, `RebaseDialog`. If either of the
     latter two renders an unsorted list, sort explicitly at that call site rather
     than reinstating it in the hook.

**Test:** `workspaceSidebarSort.test.ts` — a fixture covering pinned rows,
missing timestamps, equal timestamps, both orders, and a snapshot equality
assertion against a copy of the *old* comparator kept inline in the test file.
That last part is what proves ordering is unchanged.

---

## Step 8 — Stable row identity

**File:** `packages/web-core/src/shared/hooks/useWorkspaces.ts`

Keep a `useRef<Map<string, { ws: WorkspaceWithStatus; summary?: WorkspaceSummary; row: SidebarWorkspace }>>`.
In the mapping memo, reuse the cached `row` when both `ws` and `summary` are
reference-identical to the previous inputs for that id; otherwise rebuild. Prune
ids that are no longer present so the map cannot grow unboundedly (constraint 17).

**Test:** rendering the hook twice with the same stream/summary objects yields
`toBe`-identical row objects; changing one workspace's summary rebuilds only that
row.

---

## Step 9 — Memoise the row

**Files:** `packages/ui/src/components/WorkspaceSummary.tsx`,
`packages/ui/src/components/WorkspacesSidebar.tsx`

1. Wrap the `WorkspaceSummary` export in `React.memo` (`:59`). Keep the
   displayName so existing tests that query by component still work.
2. Replace the per-row `onClick={() => onSelectWorkspace(workspace.id)}` closures
   (`:171`, `:373`, `:490`) with a stable handler — either pass `workspace.id` and
   have the row call `onSelect(id)`, or memoise per-id handlers in the parent. Do
   the same for any other inline per-row callbacks in those three call sites.
3. Also memoise the `headerActions` array (`:245`) and, in the container, the
   `sidebarPersistKeys` object (`:614`) and `searchControls` JSX (`:620`) — all
   are recreated every render and would defeat memoisation of the sidebar itself.

**Test:** existing `packages/remote-web/src/test/*` sidebar tests must still pass;
add a render-count assertion that changing one workspace's summary re-renders one
row.

---

## Step 10 — Bound the search path

**File:** `packages/web-core/src/pages/workspaces/WorkspacesSidebarContainer.tsx`

1. Debounce `searchQuery` by 150 ms into a `debouncedSearch` used by the filter
   memos (`:403`, `:432`, `:465`), leaving the input bound to the immediate value.
   **The armed timer must survive unrelated re-renders** and read its target from
   a ref — a cleanup that clears and re-arms starves (constraint 2).
2. Drop the `isSearching ?` special-case in `paginatedActive/ArchivedWorkspaces`
   (`:525-539`) so `displayLimit` applies while searching. `handleLoadMore`
   (`:547`) already guards on `!isSearching`; remove that guard so scrolling a
   search result can load more.
3. While there: `totalWorkspacesCount` (`:691`) passes the **unfiltered** length,
   so the count beside "Active" disagrees with the rows shown under a search or
   filter. Pass the filtered length. Small, in the same file, and visibly wrong.

**Test:** filtering to >50 matches renders 50 rows and reveals more on
`handleLoadMore`; the debounce fires once for a burst of keystrokes and is not
starved by an unrelated re-render.

---

## Verification sequence

```
pnpm install --frozen-lockfile        # already done
cargo test --workspace
pnpm run prepare-db && pnpm run prepare-db:check
pnpm run generate-types:check
pnpm run check
pnpm run lint
pnpm run format
```

Then the empirical check, read-only over HTTP against the coordinator
(`http://172.16.100.102:3334`) — **do not start a dev server on this shared host**
(constraint 13):

```
POST /api/workspaces/summaries {"archived":false}   # control: 5.64 s / 5.78 s
POST /api/workspaces/summaries {"archived":true}    # control: 1.92 s / 1.67 s
```

Expected after the change: first call is DB-only (tens of ms) with `None` stats;
subsequent calls are DB-only with populated stats matching the control run's
values. Diff the two payloads field-by-field, ignoring `files_changed`/`lines_*`
for workspaces whose worktree is deleted (those legitimately move from `0` to
`null`, which renders identically — `SPEC.md` B2).

## Deviations from this plan, as built

1. **Step 2.3/2.4 (batching the `WorkspaceRepo` N+1) was dropped.** It only
   mattered because it ran 132× every 15s next to the git work; with refreshes
   capped at once per 60s per workspace and gated by a semaphore, it is an indexed
   SQLite point lookup on a background task. Batching would have needed a
   runtime-checked `IN (…)` query plus a hand-written `FromRow` for the flattened
   `RepoWithTargetBranch` — new machinery, no measurable payoff. Now a follow-up.
2. **`compute_diff_stats`'s contract was not changed.** It has eight callers, so
   instead of altering its lenient behaviour a sibling
   `compute_diff_stats_strict` was added (returns `None` if *any* repo fails) and
   only the cache uses it. Both share `repo_diff_stats`, so both got the
   `--numstat` speedup.
3. **`push`/`force push` are not invalidated** (step 6 listed them): publishing a
   branch to the remote does not change the diff against the merge base. The
   handlers that do get an explicit call are `change_target_branch`,
   `rebase_workspace`, `continue_workspace_rebase`,
   `abort_workspace_conflicts` and `merge_workspace` — see `SPEC.md` B3 for why
   each one needs it and why `rename_branch` does not.
4. **`snapshot` / `refresh_stale` / `invalidate` are `async`.** `moka::future::Cache`'s
   accessors are async, and `crates/services` only enables moka's `future`
   feature. Making them async was preferable to adding the `sync` feature; all the
   awaits are in-memory.
5. **Step 9 became a memoised wrapper row** (`SidebarWorkspaceRow` inside
   `WorkspacesSidebar.tsx`) rather than `React.memo` on `WorkspaceSummary`
   itself. `WorkspaceSummary` is used outside the sidebar (drafts, other
   surfaces); wrapping it at the three sidebar call sites keeps the change
   contained and moves the ~15-prop spread out of the render body.
6. **`CreateModeProvider` needed a fix** that step 7.3 predicted: it read
   `activeWorkspaces[0]` and so depended on the hook's ordering. It now picks its
   own head element with the old precedence preserved.
7. **The ordering-equality test found a genuine divergence.** The legacy
   comparator branched on `a.isPinned !== b.isPinned` without normalising, so
   `false` sorted ahead of `undefined`. `toSortKey` normalises both to `false`.
   Unreachable with real data (`pinned` is a non-null boolean on the wire), and
   documented in a dedicated test rather than papered over.

## Review checkpoints (things most likely to be got wrong)

1. Temp index retained in the numstat path (untracked files still counted).
2. Failed computation stored as absent, never as `0/0/0`.
3. `inflight` cleared by a guard, so failures are retried.
4. Generation checked before insert, so a stale refresh cannot clobber.
5. `Weak` handle in the watcher; `Lagged` continues rather than killing it.
6. `REFRESH_AFTER` unconditional — the backstop, not an optimisation.
7. `crates/db/.sqlx` regenerated and committed.
8. New sort helper's ordering asserted equal to the old comparator, not just
   "reasonable".
