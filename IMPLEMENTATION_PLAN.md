# Implementation Plan: Never discard uncommitted worktree work on restart

Two independent defects, in two different crates. They share a theme but no
code, so they are sequenced to land and be verified separately.

## Phase 0 — Establish the baseline

1. Run `pnpm install --frozen-lockfile` (fresh worktree requirement), then
   `cargo test --workspace` to confirm a green starting point before touching
   anything. Record which crates already have test infrastructure:
   `crates/git` has real coverage, `crates/worktree-manager` has tests as of
   #151, and `crates/workspace-manager` has **none**.

## Phase 1 — D1: preserve WIP regardless of how the process stopped

Target: `crates/local-deployment/src/container.rs`, `kill_all_running_processes`
(~line 2668).

2. Restructure the loop body so `commit_interrupted_wip` is attempted for every
   non-persistent process, independent of whether `stop_execution` succeeded.
   Keep the existing distinct logging for the kill outcome — a failed kill is
   still worth an `error!` — but stop gating preservation on it.
3. Ensure the preservation attempt runs *after* the kill attempt in both cases,
   so a still-live writer has been signalled first. Do not reorder into
   snapshot-then-kill; the KB invariant is explicitly kill-then-snapshot.
4. On preservation failure, log at `error!` with workspace id and the repo
   names that failed, per FR-2.
5. Consider (and decide explicitly) whether to also mark the execution row
   `Interrupted` when `stop_execution` bails early at the "child not found"
   branch, since it currently returns before `update_completion` and leaves the
   row `Running`. The KB page states snapshot failures must not leave a killed
   execution `Running`; the same argument applies to a kill that found no child.
   If this widens the change too far, record it as a follow-up rather than
   silently skipping it.

6. **Test (AC-1).** The honest constraint: `kill_all_running_processes` needs a
   `LocalContainerService` with a DB pool, which the existing test modules in
   this file do not construct — both `queued_follow_up_tests` and `warm_tests`
   test pure helpers and in-memory registries. So:
   - Extract the branch decision into a small pure helper (the codebase's
     established pattern — see `reset_would_discard_uncommitted_work` plus its
     truth-table test) expressing "attempt preservation for this process
     regardless of kill outcome", and unit-test it directly.
   - Additionally add a `crates/git`-level test only if a genuine gap exists
     there; `commit_interrupted_wip`'s own multi-repo behaviour is unchanged by
     this phase and should not be re-tested.
   - If a full integration test proves disproportionate, say so in the PR
     rather than claiming coverage the tests do not provide.

## Phase 2 — D2: guard the orphan sweep

Target: `crates/workspace-manager/src/workspace_manager.rs`,
`cleanup_orphans_in_directory` (~line 557).

7. Add a filesystem-based cleanliness probe. `is_container_clean` **cannot** be
   reused here: it takes a DB `Workspace` and enumerates `WorkspaceRepo` rows,
   but an orphan candidate is by definition absent from the DB. The probe must
   instead walk the candidate directory's subdirectories and, for each that
   looks like a git worktree, call `GitService::get_worktree_change_counts`.
   `workspace-manager` already depends on `git`, so no new dependency edge.
8. Treat the directory as retained if any subdir reports uncommitted or
   untracked changes. On probe error, **retain** — matching the fail-safe
   direction `cleanup_expired_workspaces` already takes, and opposite to the
   unsafe direction `reset_session_to_process` takes.
9. Per FR-4, log before acting: the path, why it was judged an orphan (no
   matching `container_ref`), and the action (deleting vs retaining, with the
   dirty counts that caused a retain). Promote the destructive leaf logging in
   `comprehensive_worktree_cleanup` from `debug!` to `info!` for the
   `remove_dir_all` and the `git worktree remove --force`, so a wipe is visible
   at default log level — the report's central complaint was that nothing was
   logged.
10. Fix the adjacent correctness bug found during investigation:
    `cleanup_workspace_without_repos` returns `Ok(())` even when its final
    `remove_dir_all` fails, so the caller logs "Successfully removed orphaned
    workspace" when nothing was removed. Propagate that error.
11. **Test (AC-2, AC-3, AC-4).** `crates/workspace-manager` has no test
    infrastructure at all, so this phase adds the first. Use `tempfile` (already
    a dev-dependency in `worktree-manager`; add it here) and real `git` command
    invocations, mirroring how `crates/git/tests/git_workflow.rs` and the #151
    tests in `worktree_manager.rs` build fixtures.
    - Dirty single-repo orphan candidate is retained.
    - Clean orphan candidate is still removed (guards against over-correcting
      into a leak).
    - Multi-repo candidate where only the *second* repo is dirty is retained.
    - Prefer testing the cleanliness predicate as a pure-ish function over a
      directory path, so the tests do not need a DB pool.

## Phase 3 — Verify honestly

12. `cargo test --workspace`, `pnpm run check`, `pnpm run lint`,
    `pnpm run format`.
13. Exercise the actual restart behaviour rather than trusting unit tests:
    create a workspace, write uncommitted + staged changes, restart the server,
    and confirm via `git status` and the log that the work survived and that the
    decision was logged. This is the reproduction from the report, and it is the
    only check that tests the thing the user actually cares about. Report the
    observed result, including if it fails.

## Phase 4 — Scope discipline

14. Do **not** re-implement #151's repair-first recreation, `.recovered-*`
    move-aside, or expiry-sweep guard. Do not bind admin dir names to workspace
    id — investigation showed cleanup matches by path, not name, so it would add
    risk for no benefit. Do not touch the `force_when_dirty` reset contract.
15. Record the deliberately-not-fixed findings (repo-wide `git worktree prune`
    side effect; `/var/tmp` base dir vs OS temp reaping; `is_container_clean`
    ignoring workspace-root files; the two divergent cleanliness helpers) in the
    PR description and the knowledge base rather than expanding this change.

## Phase 5 — Review and knowledge

16. Independent Codex review of the diff; iterate until no significant findings.
17. Update `docs/knowledge-base/interrupted-worktree-recovery.md` — it already
    states the invariant this task enforces, so it needs the kill-vs-snapshot
    independence rule added and this task's id appended — plus the INDEX row.
    Add the orphan-sweep-vs-expiry-sweep distinction, which the KB does not
    currently cover at all.
