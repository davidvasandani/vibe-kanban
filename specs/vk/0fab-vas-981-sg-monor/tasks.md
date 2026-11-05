# Tasks: Reliable workspace creation for large/foreign-hook repos

**Plan**: `./plan.md`

## Phase 1: Git-layer primitives
- [x] T001 [P] Disable hooks for every `GitCli` command in
      `crates/git/src/cli.rs` (`git_impl`: `-c core.hooksPath=/dev/null`)
- [x] T002 [P] Add `GitService::fetch_remote_branch` in `crates/git/src/lib.rs`

## Phase 2: Core
- [x] T003 Call `fetch_remote_branch` on a local branch-existence miss in
      `crates/workspace-manager/src/workspace_manager.rs` (`add_repository`;
      depends on T002)
- [x] T004 [P] Persist the real failure message instead of a placeholder in
      `crates/server/src/routes/workspaces/create.rs`
      (`create_and_start_workspace`'s background task — both the
      `claim_creation_with_retry` and `run_create_and_start_workspace`
      failure branches; the first was missed in the initial pass and added
      per the `/speckit.analyze` finding)
- [x] T005 [P] Make `create_session` branch on `workspace.creation_status`
      in `crates/server/src/routes/sessions/mod.rs`

## Phase 2.5: Rebase (branch was 390 commits behind `origin/main`)
- [x] T006b This branch (`vk/0fab-vas-981-sg-monor`) was discovered to be cut
      from a stale base — 390 commits behind `origin/main`, carrying 30
      unrelated "cluster node diagnostics" commits. Per David's decision
      (issue comment 2026-11-05 14:24 UTC), the branch (never pushed) was
      reset to `origin/main` at `182a0cb9`, and T001-T005 were reapplied
      against the current `create.rs`/`cli.rs`/`lib.rs`/`workspace_manager.rs`/
      `sessions/mod.rs` — now with `GitHubCredentials` threaded through the
      remote-branch-check/fetch calls (added upstream since this branch's
      original base) and against the constitution's current principle
      numbering (LVIII → added LIX, not XXXI)

## Phase 3: Validation
- [x] T006 `cargo check -p git -p workspace-manager -p server` (depends on
      T001-T005) — clean, no warnings from these changes
- [x] T007 [P] `cargo fmt` — reformatted only the lines this task touched
- [x] T008 [P] `cargo clippy -p git -p workspace-manager -p server --no-deps
      -- -D warnings` — clean
- [x] T009 `cargo test -p git -p workspace-manager -p server` (depends on
      T001-T005) — all passing (git: 56, workspace-manager: 31, server
      workspaces/sessions: 57); plus `cargo check --workspace` as a final
      full-workspace sanity pass
- [ ] T010 Independent review via the `codex-review` skill (depends on
      T006-T009); address confirmed findings and re-verify

<!--
T001/T002 are independent files within the same crate (parallel-safe).
T004/T005 are independent files in `server` (parallel-safe with each other
and with T001/T002, but T003 depends on T002).
-->
