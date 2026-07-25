# Research Notes

## D-1: The reported incident was already fixed before it happened

The incident is timestamped 18:25 UTC on 2026-07-25. Commit `0fb74539` ("Stop
wiping node_modules and uncommitted work on worktree recreation", #151) landed at
11:05:33 -0700 = **18:05 UTC** — twenty minutes earlier. The server process that
destroyed the work had been running since ~17:58 UTC, so it was the pre-#151
binary; the 18:25 restart was the rebuild path deploying #151 itself.

The destructive path was `resume_interrupted_coding_agents`
(`crates/server/src/startup.rs:170`) → `ensure_container_exists` →
`WorktreeManager::ensure_worktree_exists`, which pre-#151 ran `remove_dir_all` on
an intact working tree whenever git's admin linkage had drifted.

**Decision**: scope this task to what is still broken at `HEAD`, and document the
above so the same investigation is not repeated. Rejected the alternative of
re-implementing preservation in the recreation path — #151 already does it, and
duplicating it would violate principle VI.

## D-2: The obvious secondary fix is actively harmful

`kill_all_running_processes` skips `commit_interrupted_wip` when `stop_execution`
errors. The natural reading is that this is *the* bug, and that the adjacent
defect — `stop_execution` returning before `update_completion`, leaving the row
`Running` — should be fixed at the same time.

Tracing `cleanup_orphan_executions`
(`crates/services/src/services/container.rs:324`) shows the opposite.
That routine runs at startup, selects rows that are still `Running` via
`find_running`, and calls `commit_interrupted_wip` **unconditionally**. So the
missing status update is precisely what lets the next startup rescue the work.

**Decision**: fix the preservation gating; do **not** touch the row status.
Marking it `Interrupted` at shutdown would hide it from `find_running` and delete
the backstop that protects users today.

**Consequence**: the shutdown gap is defence-in-depth, not the root cause. An
earlier draft of the internal spec claimed it would alone have saved the reported
session; that claim was wrong and has been corrected in place rather than quietly
dropped.

## D-3: Startup ordering makes the orphan sweep the sharper defect

`spawn_workspace_cleanup` is `tokio::spawn`ed from `LocalContainerService::new`
(`crates/local-deployment/src/container.rs:387`), which runs inside
`DeploymentImpl::new` — called at `crates/server/src/startup.rs:155`, *before*
`cleanup_orphan_executions` at `:159`, and then concurrently with it.

Since D-2 establishes that startup WIP capture is the mechanism actually
protecting users, an unguarded `remove_dir_all` racing it is the more dangerous
of the two defects. This reprioritised the work: the orphan sweep gets the real
tests, not the shutdown path.

## D-4: Cleanliness probe — alternatives considered

The orphan candidate is absent from the DB, so `is_container_clean` (which takes
a `Workspace` and enumerates `WorkspaceRepo` rows) is unusable.

| Option | Verdict |
| --- | --- |
| Reuse `is_container_clean` | **Rejected** — needs a DB `Workspace` that does not exist by definition |
| Look up the workspace by fuzzy/canonicalised path, then reuse it | **Rejected** — changes orphan *detection*, a bigger behavioural change than the guard itself, and canonicalisation drift is a separate bug |
| Filesystem probe with `get_worktree_change_counts` per subdir | **Chosen** — no new dependency, matches the definition already used by the expiry guard |
| `GitService::is_worktree_clean` (git2) | **Rejected** — `include_untracked(false)`, so it would miss exactly the untracked new files the report describes |

### Why "no `.git` marker anywhere → deletable"

Retain-on-error (FR-4) taken alone would leak every unprobeable directory
forever. Treating a directory with no git worktree in it as deletable bounds the
leak to directories that genuinely look like worktrees but whose git is broken —
the ambiguous case where retaining is the right call. This keeps FR-6
satisfiable.

## D-5: Pooled `<repo><N>` admin directories are not a second failure mode

The report asks whether pooled admin dirs can be reassigned across restarts,
suspecting one task's teardown could clobber another's tree.

Nothing in this codebase chooses the admin dir name. `git worktree add` derives
`.git/worktrees/<name>` from the *basename* of the worktree path, which is
`repo.name` alone (`crates/workspace-manager/src/workspace_manager.rs:312`),
appending an ordinal on collision. The workspace id appears only in the parent
directory (`<4-hex-uuid>-<slug>/<repo.name>`).

But the mapping is never trusted by name: `find_worktree_git_internal_name`
(`crates/worktree-manager/src/worktree_manager.rs:340`) rediscovers it by reading
each `worktrees/*/gitdir` and canonicalising it against the target **path**.
Cleanup therefore matches by path, so a recycled ordinal cannot cause
cross-task destruction.

**Decision**: do not bind admin dir names to workspace id. It would add risk for
no benefit.

## D-6: Findings recorded but deliberately not fixed

- **Repo-wide `git worktree prune`.** `comprehensive_worktree_cleanup` ends with
  a prune (`worktree_manager.rs:414`) that operates on the whole source repo. If
  several workspaces' directories are off-disk at once, one workspace's cleanup
  drops other live workspaces' admin entries, forcing their recreation.
  Post-#151 this costs a `.recovered-*` directory rather than the data.
- **Worktree base dir lives under `/var/tmp`**
  (`worktree_manager.rs:676`). OS temp reaping removes worktree directories while
  admin entries and DB `container_ref`s survive — precisely the drifted state
  that drives recreation. This is the systemic contributor, and moving it is a
  far larger change than this fix.
- **`container_ref` matching is an exact, un-canonicalised string compare**
  (`crates/db/src/models/workspace.rs:259`), unlike
  `find_worktree_git_internal_name` which does canonicalise. Symlinked `/var/tmp`,
  a trailing separator, or a changed `workspace_dir` override can misclassify a
  live workspace as an orphan. The FR-3 guard defends against the *consequence*;
  the misclassification itself remains.
- **Two divergent cleanliness definitions.** `is_container_clean` counts
  untracked files; `check_worktree_clean` (git2) does not. Reconciling them would
  change the `force_when_dirty` reset contract, which is out of scope.
- **`is_container_clean` blind spots**: files directly in the workspace root
  outside any repo subdir are never inspected, and a missing repo subdir counts
  as clean.

All five are recorded for the knowledge base rather than expanded into this
change, per principle III.

## Dependencies

No new runtime dependency. One new dev-dependency: `tempfile` for
`crates/workspace-manager`, already used by `crates/worktree-manager` and
`crates/git` for the same fixture style — so nothing to justify under the
constitution's new-dependency constraint beyond this note.
