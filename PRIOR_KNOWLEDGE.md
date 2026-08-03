# Prior Knowledge: Workspace List Load/Sort Cost (`d49f-loading-workspac`)

The project knowledge base is populated. This repository carries **two**
knowledge bases and both were searched:

- `docs/knowledge-base/` (21 pages, `INDEX.md`) — the current one; task ids
  match `specs/vk/*`.
- `wiki/` (19 pages, `INDEX.md`) — an earlier generation, still authoritative
  for frontend and lifecycle topics.

## Most relevant pages

| Page | Why |
| --- | --- |
| `wiki/workspace-carousel-view.md` | Directly documents the 15 s summaries repoll + identity-churn problem this task fixes, and names the other consumer of the diff-stat fields (`diffStatsOverride`) |
| `wiki/browser-session-control-arbiter.md` | The exact precedent for driving cache invalidation off the SQLite-hook event stream, incl. the `Weak`-handle rule and "broadcast lag can drop events → TTL is the backstop" |
| `docs/knowledge-base/clustered-workspace-execution.md` | Which node may touch a worktree; NFS-loss = indeterminate, not idle |
| `docs/knowledge-base/workspace-directory-reclamation.md` | Absent-vs-stat-failed, "a git probe that errors is not a clean repo", and the two divergent worktree-cleanliness definitions |
| `docs/knowledge-base/mcp-oauth-connect.md` | The repo's in-memory-cache idiom: TTL **plus** an explicit capacity cap |
| `docs/knowledge-base/collapsing-repeated-log-entries.md` | Unbounded in-memory accumulation has already burned this repo; byte-identical output for existing consumers |
| `docs/knowledge-base/issue-status-side-effects.md` | Level-triggered beats edge-triggered for derived state |
| `docs/knowledge-base/worktree-formatting-prerequisites.md` | Fresh-worktree verification order and CI change-filter completeness |
| `docs/knowledge-base/interrupted-worktree-recovery.md` | Keep the decision a small pure helper with a truth-table test; a backstop that never runs is not a backstop |
| `docs/knowledge-base/mcp-connectivity-testing.md` | Bounded fan-out with per-probe timeouts; key invalidation on the exact id, never a composed string |
| `docs/knowledge-base/active-mcp-refresh.md` | A stale background task must not overwrite a newer value |
| `wiki/kanban-items-state-and-activity-grouping.md` | Which workspace signals are semantic vs display-gated |
| `wiki/electric-sync-fallback.md` | Cached-config/stable-callback traps; "degradation is not an error" |

## Hard constraints extracted for this task

### The 15 s repoll and identity churn are already documented

1. **This is a known, recorded problem.** `wiki/workspace-carousel-view.md`:
   "Workspace summaries repoll every 15s and the WS stream patches often, so the
   sort input changes identity frequently with *equal content*." The carousel
   worked around it with an `arraysEqual` content comparison rather than fixing
   the source. Stabilising row identity (spec F2) removes the *cause*; the
   carousel's content compare stays correct either way, so the two do not
   conflict — but do not "simplify" that workaround away as part of this task,
   because the WS stream still patches with equal content for other reasons.

2. **A debounce whose timer is re-armed in effect cleanup can starve forever**
   (same page). Applies to the spec's F4 search debounce: let an armed timer
   survive unrelated re-renders and read its target from a ref, rather than
   clearing and re-arming on every dep change.

3. **`diffStatsOverride` is the second consumer of these fields.** The carousel
   feeds chat-box stats from workspace summaries "instead of the diff store —
   otherwise columns show a false '0 files changed'". Verified in source: the
   readers are `WorkspaceCarouselColumn.tsx:158`, `KanbanContainer.tsx:683`,
   `IssueWorkspacesSectionContainer.tsx:95`, `ProjectRightSidebarContainer.tsx:407`
   and `packages/remote-web/.../hosts.$hostId.workspaces.tsx:109`. All of them
   either `?? 0` or gate on `!== undefined`, and the sidebar badge itself is
   `hasChanges = filesChanged !== undefined && filesChanged > 0`
   (`packages/ui/src/components/WorkspaceSummary.tsx:81`). **So `None` renders
   identically to `0`** — a cold cache is invisible, not a regression. This is
   what makes the stale-while-revalidate design safe without an API change.

### Event-driven invalidation: copy the existing watcher, keep the backstop

4. **The precedent exists and should be reused verbatim in shape.**
   `wiki/browser-session-control-arbiter.md`: "Execution-completion and
   workspace-archival cleanup subscribe to the existing SQLite-hook JSON-patch
   stream (`EventService` msg store) rather than threading the browser service
   into `LocalContainerService` finalization." The implementation to model on is
   `BrowserSessionService::spawn_cleanup_watcher`
   (`crates/services/src/services/browser/mod.rs:622-682`): take
   `Arc<MsgStore>`, `Arc::downgrade` the inner state, `tokio::spawn`, and match
   on `/workspaces/{uuid}` and `/execution_processes/{uuid}` patch paths.

5. **The stream is lossy — a TTL backstop is what makes it safe.** Same page:
   "Broadcast lag can drop events — the lease TTL and idle sweep are the
   backstop, which is what makes this design safe." Confirmed in source: the
   watcher handles `RecvError::Lagged(_)` by `continue`, i.e. dropped events are
   simply skipped. **`REFRESH_AFTER` is therefore not an optimisation knob, it is
   the correctness backstop** — it must stay short enough to be a real safety
   net and must never be made conditional on having seen an event.

6. **Hold only a `Weak` between ticks.** "The sweeper holds only a `Weak` to the
   service between ticks (a strong clone in the loop leaks the service forever —
   same class of bug as the keep-warm poll loop)."

7. **Level-triggered over edge-triggered**, from
   `docs/knowledge-base/issue-status-side-effects.md`: "This is deliberately
   **level-triggered**, not tied to the status-change event. The same comparison
   runs after Electric updates, fallback snapshots, reconnects, and provider
   remounts, so temporary disconnection does not lose the side effect." The
   `refresh_stale` sweep is the level-triggered half; invalidation is only the
   latency optimisation.

8. **A stale refresh must not clobber a fresher value.**
   `docs/knowledge-base/active-mcp-refresh.md`: the container "removes it only
   when that exact execution finishes, preventing an older cleanup task from
   removing a newer control", and "a write lock serializes request, failure, and
   confirmation transitions; readers therefore see either the previous complete
   server vector or the replacement vector." Applied: a refresh that began before
   an `invalidate` must not write its result over the invalidation, and a reader
   must never see a torn `DiffStats`. Store the whole struct atomically and
   compare-before-write against the invalidation generation.

9. **Key the cache on the workspace UUID, never a composed string.**
   `docs/knowledge-base/mcp-connectivity-testing.md`: "Match invalidation by the
   result's exact `server_name`, not a serialized-key prefix, because
   user-controlled names may contain the key delimiter." Repo and branch names
   are user-controlled here.

### Cluster / NFS: what the coordinator may do to a worktree

10. **Reading is allowed; administration is not.**
    `docs/knowledge-base/clustered-workspace-execution.md`: "Workers may run
    ordinary Git commands inside their assigned worktree, but only the
    coordinator may add, remove, prune, or reclaim worktrees and delete shared
    branches." Diff-stat computation is an ordinary read, and the coordinator is
    already authoritative for SQLite and worktree administration — so serving
    summaries centrally stays within the model. Worth flagging in the plan: the
    existing temp-index pipeline runs `git add -A`, which writes **loose objects
    into the shared object store** even though the index itself is a `/tmp` file.
    That is pre-existing behaviour; this task reduces its frequency rather than
    removing it, and removing it (`git add -N` + a non-`--cached` diff) is a
    follow-up, not a drive-by.

11. **An error is not a zero.**
    `docs/knowledge-base/workspace-directory-reclamation.md`: "A git probe that
    errors is not a clean repo", and "`Path::exists()` returns `false` for both
    'absent' and 'stat failed' — use `try_exists()`, which distinguishes them."
    Reinforced by `clustered-workspace-execution.md`: "An offline or unreachable
    worker means the workspace is indeterminate, not idle" and "An existing path
    does not prove that NFS is mounted." **Therefore the cache must not store a
    failed computation as `0/0/0`** — leave the entry absent so the field stays
    `None`. Today's code does the opposite (`compute_diff_stats` `continue`s past
    a failed repo and returns a zeroed struct), which is why cleaned-up
    workspaces currently report a confident "0 files changed".

12. **A worktree can vanish under a cached entry.** Same page: cleanup ends with
    a "**repo-wide** `git worktree prune`, so one workspace's cleanup can drop
    other live workspaces' admin entries", and "The worktree base dir lives under
    `/var/tmp`, so OS temp reaping removes working trees while admin entries and
    `container_ref`s survive". So `worktree_deleted` and `container_ref` are
    advisory, not authoritative — they are a cheap *skip* filter, never proof that
    a cached number is still valid. The TTL covers the rest.

13. **Do not run a dev server against the shared host to test this.** Same page:
    "`cleanup_orphan_workspaces` **always** sweeps the default base dir even when
    an override is configured, and with a non-matching DB it will classify every
    live worktree there as an orphan." Verification against the live coordinator
    must stay read-only HTTP (`POST /workspaces/summaries` latency measurement),
    which is how the numbers in `SPEC.md` were taken.

### Preserving the meaning of the numbers

14. **The untracked-files trap in the `--numstat` switch.**
    `docs/knowledge-base/workspace-directory-reclamation.md` records two
    divergent cleanliness definitions: the porcelain path "counts **staged and
    untracked**", while `GitService::is_worktree_clean` uses
    `include_untracked(false)` and "**misses untracked files entirely**". A bare
    `git diff --numstat <base>` is the second kind. The switch is only
    behaviour-preserving because it keeps the existing temp-index preparation
    (`read-tree HEAD` → `status --porcelain -z` → `add -A` with
    `GIT_INDEX_FILE`) and changes **only** the final command to
    `diff --cached -M --numstat`. Dropping the temp index to "simplify" would
    silently stop counting untracked files.

15. **First occurrence must stay byte-identical for existing consumers.**
    `docs/knowledge-base/collapsing-repeated-log-entries.md`. The cross-check
    test (numstat aggregate vs `get_diffs` aggregate) is the enforcement.

### Bounded caches and background work

16. **TTL *and* a hard capacity cap.** `docs/knowledge-base/mcp-oauth-connect.md`:
    "module-local `LazyLock<RwLock<HashMap<Uuid, PendingFlow>>>`, 10-min TTL
    pruned on access" and "**Pending state and token files are bounded.** At most
    256 unexpired flows are retained." The moka cache must carry both.
17. **Unbounded in-memory growth has already caused an OOM here.**
    `docs/knowledge-base/collapsing-repeated-log-entries.md`: "Never render an
    unbounded tick string… repeatedly building progressively larger replacement
    patches can exhaust the server's memory."
18. **Retain-on-error must be bounded**
    (`workspace-directory-reclamation.md`): "Without it, retain-on-error means
    retain-forever." Applied: a workspace whose refresh keeps failing must not
    keep a permanently pinned entry or a permanently held `inflight` marker —
    clear `inflight` in a guard/`finally`-equivalent, not on the success path.
19. **Bounded fan-out with explicit caps is established practice**
    (`mcp-connectivity-testing.md`, `aws-sso-profile-management.md`,
    `cli-tool-oauth-login.md`): "probes concurrently (`futures::future::join_all`),
    each probe wrapped in `tokio::time::timeout`", "cap concurrent role
    requests", `kill_on_drop`. Note the deliberate deviation to record in the
    plan: the git work runs inside `spawn_blocking`, which **cannot** be
    cancelled by `tokio::time::timeout` — a timeout would release the semaphore
    permit while the thread stayed stuck on NFS. The semaphore itself (permit
    held for the whole blocking call) plus the `inflight` dedupe is the bound.
20. **Never make the request wait on slow work**
    (`docs/knowledge-base/remote-external-integrations.md`): "an ack must never
    wait on the DB or an outbound API call. Ack immediately and do the work in
    `tokio::spawn`." That is precisely the stale-while-revalidate shape.

### Verification and CI

21. **`pnpm install --frozen-lockfile` before anything else in a fresh
    worktree**, and "Do not assume `node_modules/.bin/prettier` exists at the
    repository root" (`worktree-formatting-prerequisites.md`).
22. **Change filters must cover new files.** Same page: "Also include the checker
    and its test paths in CI change filters. Adding a test command to a filtered
    job is insufficient if changes to the tested files do not trigger that job."
    Verified in `.github/workflows/test.yml:41-92`: every path this task touches
    (`crates/db`, `crates/deployment`, `crates/git`, `crates/local-deployment`,
    `crates/server`, `crates/services`, `packages/ui`, `packages/web-core`) is
    already covered, so **no filter change is needed** — but this was checked, not
    assumed.
23. **The SQLx offline cache is committed and CI enforces it.** Verified in
    source (the knowledge base only documents the *remote*/Postgres flow):
    `crates/db/.sqlx` is tracked (388 files) and `.github/workflows/test.yml:147`
    runs `npm run prepare-db:check`. Changing the `find_all_with_status` SQL
    **requires** re-running `pnpm run prepare-db` and committing the result, or
    CI fails.
24. **A perf test that passes before the fix proves nothing**
    (`workspace-directory-reclamation.md`): "Pair it with a control run on the
    unfixed code." The `SPEC.md` table is that control run.
25. **Keep the decision logic a pure helper with a truth-table test**
    (`interrupted-worktree-recovery.md`) — i.e. "should this workspace be
    refreshed?" and the sort comparator both belong in testable pure functions.
26. **A backstop needs something to trigger it** (same page): "the backstop needs
    a next startup to happen at all." Here the trigger is the client's own 15 s
    poll calling `refresh_stale`. If no client is connected, nothing refreshes —
    which is correct and desirable, but means the cache is only ever as fresh as
    the last poll, and a test must not assume a self-driving timer exists.

## Gaps the knowledge base did not cover (closed from source for this task)

- `EventService` / `MsgStore` semantics — undocumented in the KB; read from
  `crates/services/src/services/events.rs` and
  `crates/services/src/services/browser/mod.rs:622`. Findings folded into
  constraints 4–6 above and worth writing back in stage 5.
- Local SQLite migration conventions and the `.sqlx` CI check — only the remote
  Postgres flow was documented; see constraint 23.
- `worktree_deleted` semantics — undocumented; see constraint 12.
- The consumer set for the summaries payload — undocumented; enumerated in
  constraint 3.
