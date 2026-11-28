# Tasks: Serialized, time-bounded branch mirror (VAS-1140)

Input: `plan.md`, `spec.md`, `research.md` and `data-model.md` in this
directory. `[P]` marks tasks that touch independent files and can run in
parallel. `[USn]` marks the user story a task serves.

## Phase 1: Foundation

- [x] T001 [US4] Add `Registration`, `RegistrationState`, `registrations()`,
  `registration_state()` and `remove_stale_registration()`, with unit tests
  covering live, locked, removed, re-linked, missing-mount and unreadable
  entries. File: `crates/utils/src/worktree_linkage.rs`.

## Phase 2: SourceMirror (US1, US2, US3), depends on T001

- [x] T002 [US1][US2][US3] Create `SourceMirror`. It holds the config and
  `from_env`, the slots, `request`, the leader loop, `run_git_bounded` (process
  group, timeout, killpg), classification, backoff and `failure()`. Add tests:
  - in sync, so no spawn;
  - behind, so pushed;
  - live holder, so deterministic with no spawn and backing off;
  - stale holder, which is removed and then pushed;
  - hang, killed with its grandchild;
  - coalescing (at most 2 spawns, at most 1 concurrent);
  - deterministic with the same tip, so `Unchanged`.

  Files: `crates/workspace-manager/src/source_mirror.rs`,
  `crates/workspace-manager/src/lib.rs`,
  `crates/workspace-manager/Cargo.toml` (only if serde, chrono or ts-rs is
  needed).
- [x] T003 [US1] Route `mirror_branch_back` through
  `SourceMirror::global().request`. File:
  `crates/workspace-manager/src/shared_repository.rs`.

## Phase 3: Registration cleanup (US4), depends on T001

- [x] T004 [US4] In `adopt`, remove the old source admin dir after a verified
  adoption. Add `forget_source_registrations(repo, worktree_path)` and extend
  the adoption test. File: `crates/workspace-manager/src/shared_repository.rs`
  (after T003, same file).
- [x] T005 [US4] In `cleanup_workspace`, call `forget_source_registrations` for
  clustered inputs. In `spawn_workspace_cleanup`, run the one-time startup
  sweep with the live-workspace guard. File:
  `crates/local-deployment/src/container.rs`.

## Phase 4: UI surface (US3), depends on T002

- [x] T006 [US3] Add `source_mirror_error` to `RepoBranchStatus` and fill it in
  `get_workspace_branch_status`. Register the types. Files:
  `crates/server/src/routes/workspaces/git.rs`,
  `crates/server/src/bin/generate_types.rs`.
- [x] T007 [US3] Run `pnpm run generate-types`. File: `shared/types.ts`
  (generated).
- [x] T008 [P] [US3] Map the field in `deriveRepoInfos` and add a Vitest. Files:
  `packages/web-core/src/pages/workspaces/gitPanelRepoInfo.ts`,
  `packages/web-core/src/pages/workspaces/gitPanelRepoInfo.test.ts`.
- [x] T009 [P] [US3] Add a `RepoInfo.sourceMirrorError` field, pass it through
  `GitPanel`, render it in `RepoCard`, and add the i18n keys. Files:
  `packages/ui/src/components/GitPanel.tsx`,
  `packages/ui/src/components/RepoCard.tsx`, and the locale `tasks.json`
  files.

## Phase 5: Large-commit guard (US5), independent

- [x] T010 [P] [US5] Add `GitService::commit_added_size(path)` with a test. File:
  `crates/git/src/lib.rs`.
- [x] T011 [US5] Add the warn-only threshold check in `commit_repos`. File:
  `crates/local-deployment/src/container.rs` (after T005, same file).

## Phase 6: Verify

- [x] T012 Run `cargo fmt`, then clippy and tests for utils, workspace-manager,
  git, local-deployment and server, then `pnpm run check`, `pnpm run lint` and
  `pnpm run format`.

## Dependencies

- T001 comes before T002, T004 and T005.
- T002 comes before T003, which comes before T004.
- T002 comes before T006, then T007, then T008 and T009.
- T010 comes before T011.
- T005 comes before T011, because both edit the same file.
- Everything comes before T012.

## Implementation notes

- T008: the Vitest was added to the existing
  `GitPanelContainer.test.ts`, which already covers `deriveRepoInfos`,
  instead of a new `gitPanelRepoInfo.test.ts` file.
- T010: implemented as `GitCli::head_commit_added_size` (`crates/git/src/cli.rs`),
  because it needs `cat-file` with stdin, which is private to `GitCli`. The
  test is in `crates/git/tests/git_ops_safety.rs`.
- T006: the wire type `SourceMirrorError` lives in the server route module,
  so `workspace-manager` gains no ts-rs or serde dependency.
- T002: the group kill is `utils::process::kill_process_group_now`
  (analyze finding 3).
