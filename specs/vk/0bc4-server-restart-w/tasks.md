# Tasks: Never discard uncommitted worktree work on restart

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes.

The two changes are independent (different crates, no shared code), so T002 and
T010 could in principle proceed together; they are listed serially because
Change 2 carries the real test burden and should not be interleaved with an
untested control-flow edit.

## Phase 1: Setup

- [x] T001 [P] Add `tempfile` as a dev-dependency in
      `crates/workspace-manager/Cargo.toml` (matches the existing
      `crates/worktree-manager` dev-dependency; needed by T012–T015)

## Phase 2: Change 1 — preserve WIP regardless of stop outcome (FR-1, FR-2)

- [x] T002 Restructure the per-process loop in `kill_all_running_processes` so
      the `stop_execution` outcome is logged without producing control flow, and
      `commit_interrupted_wip` is called from a **single unconditional call
      site** after it, in `crates/local-deployment/src/container.rs` (~line 2668).
      Preserve stop-before-preserve ordering.
- [x] T003 Raise the preservation-failure log to `error!` and include the
      workspace id alongside the process id, in
      `crates/local-deployment/src/container.rs` (depends on T002)
- [x] T004 Add a comment at the `stop_execution` early-return recording **why**
      the `Running` row is deliberately left alone (spec C-1: it is what lets
      `cleanup_orphan_executions` rescue the process), in
      `crates/local-deployment/src/container.rs` (~line 2334). This is the
      guard against a future "obvious" fix that would delete the backstop.

## Phase 3: Change 2 — cleanliness probe (FR-3, FR-4)

- [x] T010 Add the crate-private probe resolving the plan's decision table —
      no `.git` marker anywhere → deletable; probe success → use staged+untracked
      counts; probe failure → `Err` (retain) — using
      `GitService::get_worktree_change_counts`, in
      `crates/workspace-manager/src/workspace_manager.rs`
- [x] T011 Wire the probe into `cleanup_orphans_in_directory` between the
      `container_ref_exists` check and `cleanup_workspace_without_repos`,
      retaining on both dirty and indeterminate results, in
      `crates/workspace-manager/src/workspace_manager.rs` (depends on T010)

## Phase 4: Change 2 — instrumentation and correctness (FR-5, FR-7)

- [x] T020 Log path, selection reason ("no workspace record referenced it"), and
      action before acting — including dirty counts on retain — in
      `cleanup_orphans_in_directory`, in
      `crates/workspace-manager/src/workspace_manager.rs` (depends on T011)
- [x] T021 Propagate the failing final `remove_dir_all` instead of swallowing it
      into `debug!` and returning `Ok(())`, so the caller stops logging
      "Successfully removed orphaned workspace" when nothing was removed, in
      `cleanup_workspace_without_repos`, in
      `crates/workspace-manager/src/workspace_manager.rs` (FR-7)
- [x] T022 [P] Promote the `git worktree remove --force` and `remove_dir_all`
      log lines from `debug!` to `info!` in `comprehensive_worktree_cleanup`, in
      `crates/worktree-manager/src/worktree_manager.rs` (~line 385). Keep the
      messages terse — this function is also on the normal recreation path.

## Phase 5: Tests

- [x] T012 Add the first `#[cfg(test)]` module to
      `crates/workspace-manager/src/workspace_manager.rs`, with a `tempfile` +
      real-`git` fixture helper building a workspace dir containing N repo
      worktrees (mirrors the fixture style of `crates/git/tests/git_workflow.rs`)
      (depends on T001, T010)
- [x] T013 [P] **AC-2**: a candidate whose repo has uncommitted *and* untracked
      changes is reported as holding unsaved work, in
      `crates/workspace-manager/src/workspace_manager.rs` (depends on T012)
- [x] T014 [P] **AC-3**: a clean candidate is reported as deletable, and a
      candidate with no `.git` marker anywhere is also deletable — guarding
      against over-correcting into a disk leak, in
      `crates/workspace-manager/src/workspace_manager.rs` (depends on T012)
- [x] T015 [P] **AC-4**: a multi-repo candidate whose **second** repo is the
      dirty one is reported as holding unsaved work, in
      `crates/workspace-manager/src/workspace_manager.rs` (depends on T012)
- [x] T016 [P] Assert the staged-changes case explicitly (`git add` without
      commit), since the reported incident lost staged files and the two
      cleanliness helpers in this codebase disagree about them, in
      `crates/workspace-manager/src/workspace_manager.rs` (depends on T012)

## Phase 6: Verification

- [x] T030 Run `cargo test --workspace` and confirm it passes (baseline was
      green before starting)
- [x] T031 Run `pnpm run check` and `pnpm run lint`
- [x] T032 Run `pnpm run format` (constitution constraint)
- [x] T033 **AC-5**: reproduce the report end-to-end. **Done, with a control
      experiment.** Method and result:

      A full server run was **rejected as unsafe**: `cleanup_orphan_workspaces`
      always sweeps the *default* base dir even when an override is set, and on
      this host that is `/var/tmp/vibe-kanban/worktrees`, holding 10+ live
      worktrees (including this one and the incident's `81fe-setup-hermes-on`).
      A server with a non-matching DB would classify them all as orphans.
      Debug builds instead use `/var/tmp/vibe-kanban-dev/`, which did not
      exist — so the real `cleanup_orphan_workspaces` was driven against that
      isolated dir via a temporary example binary (real `DBService`, real code
      path, since removed).

      Fixture mirrored the incident: a repo with a `git add`-ed new file plus a
      modified tracked file, and a second, fully-clean workspace.

      - **With the fix**: dirty workspace **retained**, contents intact, logged
        `Retaining workspace …: no workspace record references it, but it holds
        unsaved work (2 uncommitted, 0 untracked in 'homelab')`. Clean workspace
        still removed — no reclaim regression.
      - **Control, guard reverted to `HEAD`**: same fixture **destroyed**, the
        log reading only `Found orphaned workspace` → `Successfully removed
        orphaned workspace`. Precisely the reported signature: clean tree, work
        gone, nothing in the log saying so.

      Confirms the defect is real at `HEAD`, that the fix removes it, and that
      FR-6 does not regress.

## Phase 7: Review and knowledge

- [x] T040 Run an independent Codex review of the diff; address confirmed
      findings and re-verify (pipeline stage 11)
- [x] T041 Update `docs/knowledge-base/interrupted-worktree-recovery.md` with the
      kill-vs-snapshot independence rule and the C-1 warning that the `Running`
      row is load-bearing; append this task id
- [x] T042 Add the orphan-sweep-vs-expiry-sweep distinction and the D-6
      recorded-but-unfixed findings to the knowledge base, and update
      `docs/knowledge-base/INDEX.md` (depends on T041)

<!--
Conventions:
- `T001` … task ids are stable and referenced by the dependency graph.
- `[P]` … parallel-safe (independent files). Omit for tasks that must be serial.
- `[ ]` / `[x]` … completion checkbox, toggled from the workbench.
-->
