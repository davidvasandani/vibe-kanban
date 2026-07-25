# SPEC: Never discard uncommitted worktree work on server restart

## Context

A running agent session lost every uncommitted change when
`vibe-kanban-dev.service` restarted underneath it. The worktree came back clean
at the base commit, and nothing in the log said it happened.

This spec scopes the **remaining** defects at `HEAD`. Much of the originally
reported failure mode was already fixed 20 minutes *before* the incident, so
scoping is the most important part of this document.

### What was already fixed by #151 (`0fb74539`, 2026-07-25 11:05 -0700 = 18:05 UTC)

The incident occurred at **18:25 UTC**, but the server process that destroyed
the work had been running since ~17:58 UTC — i.e. it was the **pre-#151
binary**. The 18:25 restart was the rebuild path deploying #151 itself. So the
observed wipe was caused by code that no longer exists.

#151 landed, and `HEAD` retains, all of:

- **Repair-first worktree recreation.** `recreate_worktree_internal`
  (`crates/worktree-manager/src/worktree_manager.rs:119`) now calls
  `try_repair_worktree_in_place` before any destructive step, so drifted git
  admin linkage is reconnected in place instead of triggering
  `remove_dir_all` on an intact working tree.
- **Move-aside instead of delete.** When repair genuinely cannot reconnect,
  `preserve_worktree_dir_if_valuable` (`worktree_manager.rs:154`, `:283`) moves
  the directory to a sibling `<name>.recovered-<epoch>` if it holds
  uncommitted/untracked changes or an installed `node_modules`.
- **Expiry-sweep guard.** `cleanup_expired_workspaces`
  (`crates/local-deployment/src/container.rs:707`) consults `is_container_clean`
  and retains any expired workspace with pending work, retaining on error too.

**These are not to be re-implemented.** The report's asks about worktree
recreation, `.recovered-*` preservation, and expiry cleanup are satisfied.

### Ruled out by the report, and confirmed here

- **Not the `force_when_dirty` reset path.** `reconcile_worktree_to_commit`
  logs before discarding; that string never appeared.
- **Not a project cleanup script.** `repos.cleanup_script` is `NULL`.
- **Not the "orphaned file cleanup".** Despite the suggestive log line at
  `crates/local-deployment/src/lib.rs:156`, `delete_orphaned_files`
  (`crates/services/src/services/file.rs:133`) deletes **DB-tracked attachment
  blobs from the cache dir**. It never touches worktrees. This suspect named in
  the report is exonerated.

## The two real remaining defects

### D1 — Interrupted WIP is preserved only on the success path (primary)

`kill_all_running_processes` (`crates/local-deployment/src/container.rs:2668`):

```rust
if let Err(error) = self.stop_execution(&process, ExecutionProcessStatus::Interrupted).await {
    tracing::error!("Failed to cleanly kill running execution process {:?}: {:?}", process, error);
} else {
    tracing::info!("Successfully killed process: id={}", process.id);
    if let Err(error) = self.commit_interrupted_wip(&process).await { ... }   // <-- success branch ONLY
}
```

`commit_interrupted_wip` — the entire WIP-preservation mechanism added by #122 —
sits inside the `else`. When `stop_execution` returns `Err`, uncommitted work is
**silently left unpreserved**.

The incident log records exactly that error:

```
Failed to cleanly kill running execution process … Other(Child process not found for execution)
```

`stop_execution` (`container.rs:2334`) returns that error when there is no
in-memory child handle *and* no adopted pgid — the common case when the child
already died with, or just before, the server. So the single most likely
real-world restart shape is precisely the one that skips preservation.

**Severity, stated accurately.** An earlier draft of this spec claimed D1 alone
would have saved the reported session. Further tracing showed that is wrong, and
the correction is worth recording because it inverts an obvious "fix".

That early `return Err` happens *before* `ExecutionProcess::update_completion`,
so the row stays marked `Running`. The next startup's `cleanup_orphan_executions`
(`crates/services/src/services/container.rs:324`) selects exactly those rows and
calls `commit_interrupted_wip` **unconditionally** — correctly, unlike the
shutdown path. So today the shutdown gap is usually rescued one restart later,
*because* of the missing status update.

Two consequences:

1. D1 is a **defence-in-depth** fix, not the root cause. It still matters: the
   backstop only fires if there is a next startup (not true for a host shutdown
   or an uninstall), and it works by accident of an early return rather than by
   design.
2. **Do not "fix" the `Running` row at the same time.** Marking it `Interrupted`
   at shutdown would hide it from `find_running` and delete the backstop. See
   C-1 in the feature spec.

### D2 — Orphan-workspace cleanup deletes unconditionally

`cleanup_orphan_workspaces` (`crates/workspace-manager/src/workspace_manager.rs:538`)
runs at **every** startup and was **not** touched by #151. It scans the worktree
base dir and, for any directory whose path is not present as a
`workspaces.container_ref`, calls `cleanup_workspace_without_repos`, which ends
in an unconditional:

```rust
tokio::fs::remove_dir_all(workspace_dir)
```

There is **no** uncommitted-work check, **no** running-execution check, and no
log of what is about to be destroyed beyond the bare path. Its safety rests
entirely on `container_ref_exists` being an exact string match on an absolute
path. Any normalisation drift (symlinked `/var/tmp`, a changed `workspace_dir`
config override, a trailing separator) reclassifies a live workspace as an
"orphan" and deletes a whole session's work with no recovery path.

It does fail safe on DB error (`if let Ok(false)`) and does iterate per-repo
subdirectories, so the multi-repo layout is structurally handled.

**D2 races the very routine that would have saved the work.** The orphan sweep is
`tokio::spawn`ed from `LocalContainerService::new`
(`crates/local-deployment/src/container.rs:387`), which runs inside
`DeploymentImpl::new` — i.e. it is launched *before* `cleanup_orphan_executions`
is called on the main startup path (`crates/server/src/startup.rs:155` vs
`:159`), and then runs concurrently with it. So if a live workspace is ever
misjudged as an orphan, its directory can be deleted while the WIP capture that
would have rescued it is still pending. Given C-1 establishes that startup
capture is the mechanism actually protecting users today, an unguarded sweep
racing it is the sharper of the two defects.

## Answers to the report's open investigation questions

These were asked in the report and are resolved by inspection. They need
**documenting, not fixing**.

**What actually destroyed the tree in the incident?** Startup calls
`resume_interrupted_coding_agents` (`crates/server/src/startup.rs:170`) →
`ensure_container_exists` → `WorktreeManager::ensure_worktree_exists`. Pre-#151
this ran `remove_dir_all` on an intact working tree whenever git's admin linkage
had drifted. That is the destructive path, it is on the restart critical path,
and #151 fixed it. Note the same path is reachable from log normalisation
(`crates/services/src/services/container.rs:1195`) — merely opening a
workspace's log view can trigger a worktree recreation.

**Can pooled `<repo><N>` admin dirs be reassigned to a different task?** The
admin dir name is **not** bound to workspace id — nothing in this codebase
chooses it. `git worktree add` derives `.git/worktrees/<name>` from the
*basename* of the worktree path, which is `repo.name` alone
(`crates/workspace-manager/src/workspace_manager.rs:312`), appending an ordinal
on collision. The workspace id appears only in the *parent* directory
(`<4-hex-uuid>-<slug>/<repo.name>`).

However, **this is not the second failure mode the report feared.** The mapping
is never stored or trusted by name: `find_worktree_git_internal_name`
(`crates/worktree-manager/src/worktree_manager.rs:340`) rediscovers it by
reading each `worktrees/*/gitdir` and canonicalising it against the target
*path*. Cleanup therefore matches by path, not by ordinal, so a recycled name
cannot make one task's teardown clobber another's tree. Binding names to
workspace id is unnecessary.

**A real cross-workspace hazard does exist, but is second-order.**
`comprehensive_worktree_cleanup` finishes with a repo-wide `git worktree prune`
(`worktree_manager.rs:414`). If several workspaces' worktree directories are
off-disk at once, one workspace's cleanup prunes *other* live workspaces' admin
entries, so they fail `is_worktree_properly_set_up` and get force-recreated.
Post-#151 that costs a `.recovered-*` directory rather than the data. Worth
knowing; not worth destabilising cleanup for now.

**Why does this happen at all?** The default worktree base dir is
`get_vibe_kanban_temp_dir().join("worktrees")` — i.e. **under `/var/tmp`**
(`worktree_manager.rs:676`). OS temp reaping can remove worktree directories
while `.git/worktrees/*` admin entries and DB `container_ref`s survive. That is
precisely the drifted state that drives the recreate path.

**Is the multi-repo layout handled?** Yes, in both routines.
`is_container_clean` enumerates `WorkspaceRepo::find_repos_for_workspace` and
joins each `repo.name` under `container_ref`. `cleanup_workspace_without_repos`
iterates the parent's subdirectories. Neither mistakes `container_ref` for a
single git repo. Two gaps worth noting: `is_container_clean` never inspects
files sitting directly in the workspace root outside any repo subdir, and it
treats a missing repo subdir as clean.

## Goals

- G1: Uncommitted agent work is never silently discarded by a restart.
- G2: When work cannot be preserved, that fact is loud in the log.
- G3: Destructive cleanup names the workspace and path *before* acting.

## Non-goals

- Re-implementing anything from #151.
- UI surfacing of the loss (tracked separately in VAS-104).
- Changing the rebuild/restart trigger itself.

## Requirements

**FR-1 — Preserve WIP regardless of how the process stopped.**
`kill_all_running_processes` must attempt `commit_interrupted_wip` for every
non-persistent process it handles, on both the success and the error branch of
`stop_execution`. A failure to kill must not imply a failure to preserve — the
error case is the one where the working tree is most likely to still hold
unsaved work.

**FR-2 — Preservation failures are loud.** When `commit_interrupted_wip` fails,
log at `error` with workspace id and repo name, stating that uncommitted work
may be at risk.

**FR-3 — Orphan cleanup refuses to destroy pending work.** Before deleting a
directory judged orphaned, `cleanup_orphans_in_directory` must skip it when any
contained repo has uncommitted or untracked changes. If cleanliness cannot be
determined, retain — mirroring the fail-safe stance `cleanup_expired_workspaces`
already takes.

**FR-4 — Destructive cleanup is instrumented.** Log the workspace path, the
reason it was judged an orphan, and the action taken, before acting.

**FR-5 — No regression in reclaiming genuinely dead workspaces.** A clean,
truly orphaned directory must still be collected, or the worktree base dir grows
without bound.

## Acceptance criteria

- AC-1: With a running coding-agent process whose child handle is absent,
  `kill_all_running_processes` still produces a WIP commit for each repo with
  changes. Regression test.
- AC-2: An orphan-looking workspace dir containing uncommitted or untracked
  changes survives `cleanup_orphan_workspaces`. Regression test.
- AC-3: A clean orphan dir is still removed. Regression test.
- AC-4: Multi-repo workspaces are handled by both paths — a dirty *second* repo
  is enough to retain the workspace.
- AC-5: `cargo test --workspace`, `pnpm run check`, `pnpm run lint` pass.

## Risks

- Committing WIP on the error path could produce a commit for a process that is
  somehow still alive and writing. Accepted: a spurious extra commit is
  recoverable; lost work is not. This is the same trade #122 already made.
- Adding a git status call per candidate directory slows startup cleanup
  slightly. Bounded by the number of directories in the worktree base dir.
