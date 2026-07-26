# Workspace directory reclamation: two sweeps, one fail-safe rule

Tags: `0bc4-server-restart-w`

Vibe Kanban reclaims workspace directories through **two separate sweeps** that
are easy to confuse. They select different victims, run on different schedules,
and have historically been guarded to different standards. Fixing one does not
fix the other.

| | Expiry sweep | Orphan sweep |
| --- | --- | --- |
| Entry point | `cleanup_expired_workspaces` (`crates/local-deployment`) | `cleanup_orphan_workspaces` (`crates/workspace-manager`) |
| Selects | Workspaces **in** the DB whose expiry has passed | Directories **absent** from the DB |
| Runs | Every 30 min (and once at boot) | Once per boot, before the loop |
| Knows the workspace | Yes — has a `Workspace` row and its repos | No — only a path |

Both must refuse to destroy uncommitted work. Because the orphan sweep has no DB
record, it **cannot** reuse `is_container_clean` (which takes a `Workspace` and
enumerates `WorkspaceRepo` rows); it needs a filesystem-only probe over the
candidate's subdirectories.

## Orphan status is a string match, so the guard is the real protection

A directory is "orphaned" iff its path does not exactly equal any
`workspaces.container_ref` — an un-canonicalised string compare, unlike
`find_worktree_git_internal_name`, which does canonicalise. Symlinked base dirs,
a trailing separator, or a changed `workspace_dir` override can therefore
reclassify a **live** workspace as abandoned. Treat the cleanliness guard as the
thing standing between that misclassification and data loss, not as a
belt-and-braces extra.

## Fail-safe direction, and why "indeterminate" is the subtle part

Retain on dirty **and** on indeterminate. The traps are the ways Rust quietly
turns "I could not tell" into "there is nothing here":

- `read_dir(..).filter_map(|e| e.ok())` silently drops unreadable entries.
- `Path::exists()` returns `false` for both "absent" and "stat failed" — use
  `try_exists()`, which distinguishes them.
- A git probe that errors is not a clean repo.

Any of these can let a workspace whose repo was never actually inspected be
deleted despite a nominal retain-on-error policy. An independent review caught
exactly this after the first implementation looked correct.

**Bound the resulting leak** with one rule: a directory containing *no* `.git`
marker in any subdirectory holds no git work and stays deletable. Without it,
retain-on-error means retain-forever and reclamation stops working entirely.

Note the fail-safe direction is **not** consistent across the codebase:
`cleanup_expired_workspaces` treats an error as "keep" (safe), while
`reset_session_to_process` treats an error as "not dirty" (unsafe). When adding
a new decision, match the safe sibling.

## Cleanliness has two definitions here — pick the strict one

- `is_container_clean` → `get_worktree_change_counts` (git CLI porcelain):
  counts **staged and untracked**. Excludes `.git`/`node_modules` by pathspec.
- `GitService::is_worktree_clean` / `check_worktree_clean` (git2):
  `include_untracked(false)` — **misses untracked files entirely**.

Retention decisions must use the first. The second is reachable only through the
non-forced reset guard. A worktree the first calls dirty (untracked-only) still
passes the second.

## Destructive steps must be visible at default log level

The originating incident was diagnosed from filesystem mtimes and reflogs
because every destructive step logged at `debug!`. `remove_dir_all` and
`git worktree remove --force` now log at `info!`, and the sweep logs the path,
*why* the directory was selected, and the action, **before** acting. A cleanup
that returns `Ok(())` after a failed removal is worse than useless — it made the
caller log "Successfully removed orphaned workspace" for a directory still on
disk.

## Verifying changes here safely

Do not run a dev server to test the orphan sweep on a shared host.
`cleanup_orphan_workspaces` **always** sweeps the default base dir even when an
override is configured, and with a non-matching DB it will classify every live
worktree there as an orphan.

Debug builds resolve to `/var/tmp/vibe-kanban-dev` + `~/.vibe-kanban-dev`, which
is isolated from the release `/var/tmp/vibe-kanban`. Driving the real sweep
against the debug base dir gives a genuine end-to-end test with no blast radius.
Pair it with a control run on the unfixed code — a retention test that also
passes before the fix proves nothing.

## Related

- [[interrupted-worktree-recovery]] — the WIP-snapshot side of restart safety,
  and why a failed stop must not skip preservation.
- Worktree *recreation* (repair-first, `.recovered-<epoch>` move-aside) is a
  third destructive path, fixed separately in #151. It races this sweep: the
  orphan sweep is spawned from `LocalContainerService::new`, i.e. **before**
  `cleanup_orphan_executions` runs on the startup path.

## Known-unfixed, recorded deliberately

- `comprehensive_worktree_cleanup` ends with a **repo-wide** `git worktree
  prune`, so one workspace's cleanup can drop other live workspaces' admin
  entries and force their recreation.
- The worktree base dir lives under `/var/tmp`, so OS temp reaping removes
  working trees while admin entries and `container_ref`s survive — the drifted
  state that drives recreation in the first place.
- `is_container_clean` never inspects files sitting directly in the workspace
  root outside a repo subdir, and counts a missing repo subdir as clean.
- Pooled `.git/worktrees/<repo><N>` admin names are **not** bound to workspace
  id (git derives them from the path basename), but cleanup resolves them by
  path, so a recycled ordinal cannot cause cross-workspace destruction. No fix
  needed — verified, not assumed.
