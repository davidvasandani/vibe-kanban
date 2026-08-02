# Tasks: Portable Git worktrees for cluster-placed workspaces

**Plan**: `./plan.md` · **Contracts**: `./contracts/internal-seams.md` ·
**Analysis**: `./analysis.md`

Tasks are ordered by dependency. **[P]** means the task touches no file that a
task beside it touches. Paths are relative to the repository root
(`vibe-kanban/`).

> Phase order is 1 → 2 → 3 → 4 → 5 → 6, revised from the first draft after
> `analysis.md` found four ordering hazards. In particular, namespace
> containment (Phase 2) now precedes store adoption (Phase 3), because the task
> that consolidates every workspace of a repo onto one store is what makes the
> existing cleanup paths cluster-wide.

> **Environment.** Source `../../../../TOOLCHAIN_ENV.sh` (workspace root) before
> any `cargo` command; this worker has no package manager and no Rust on `PATH`
> by default. `vibe-kanban-tauri` is excluded from every cargo invocation, as CI
> does. Git does not work in this worktree — that is the bug being fixed.
> `pnpm`/`npm` are **not available here**; see Phase 7 for what that means.
>
> Do not start a dev server on a cluster host. (The orphan sweep is inert while
> clustering is enabled — `workspace_manager.rs:714-719` — so it is not the
> hazard the first draft claimed, but a second coordinator against the live
> database is still a bad idea.)

## Phase 0: Baseline

- [x] T001 Record a green baseline `cargo check --workspace --exclude vibe-kanban-tauri --all-targets`. No files change.

## Phase 1: Portability probe (contract 1)

- [x] T002 Add `WorktreeLinkage`, `LinkageStatus`, `LinkageError` in `crates/utils/src/worktree_linkage.rs`; register in `crates/utils/src/lib.rs`. **`crates/utils`, not `crates/git`** — `crates/worker` depends on `utils` and must not gain `git2`. `probe()` is pure filesystem (no subprocess): reads `.git`, the `gitdir:` line, `commondir`, and the back-pointer. `try_exists()` throughout; no `filter_map(|e| e.ok())`.
- [x] T003 Unit tests in `crates/utils/src/worktree_linkage.rs`: valid `.git` file; missing target; target outside root; real `.git` directory; no `.git`; symlink escape; worktree nested under an ancestor repository (must not be `Portable`); unreadable `.git` yields `Indeterminate`, never `Portable`.
- [x] T004 [P] Add the genuinely missing crates to the `backend` change filter in `.github/workflows/test.yml`: `crates/workspace-manager/**`, `crates/worktree-manager/**`, `crates/worker/**`, `crates/cluster-protocol/**`. (`crates/git/**` and `crates/utils/**` are already listed.)

## Phase 2: Contain the shared `worktrees/` namespace

> Lands before anything consolidates the namespace.

- [x] T005 Fix the two error-swallowing idioms in `WorktreeManager::find_worktree_git_internal_name` (`crates/worktree-manager/src/worktree_manager.rs:583-585,599`): `read_dir(...).filter_map(|e| e.ok())` and `gitdir_path.exists()`. An NFS read failure must surface as an error, not as `Ok(None)`, which currently makes the caller fall through to broader cleanup. The function's path resolution is already correct and is kept (constitution VI).
- [x] T006 Scope `comprehensive_worktree_cleanup`'s trailing repo-wide `git worktree prune` to the worktree being cleaned, in `crates/worktree-manager/src/worktree_manager.rs`. (depends on T005)
- [x] T007 Test in `crates/worktree-manager/src/worktree_manager.rs`: two worktrees of one repository; cleaning one leaves the other's registration *and* directory intact. This is the test that would have caught the 2026-07-05 prune incident. (depends on T006)

## Phase 3: Shared repository store (contract 2)

- [x] T008 Add the genuinely new Git plumbing to `crates/git/src/cli.rs` and `crates/git/src/lib.rs`: bare clone, config set, and `cat-file -e <sha>^{commit}` object-existence proof. Reuse what exists — `GitCli::{list_worktrees, fetch_with_refspec, list_remotes}` (`cli.rs:318,365,455`) and `GitService::list_remotes` (`lib.rs:1463`) — rather than reimplementing them.
- [x] T009 Add `crates/workspace-manager/src/shared_repository.rs` with `SharedRepositoryStore::new(shared_root, locks)` and `path_for(repo_id)` delegating to `SharedWorkspacePaths::repository_dir`; register in `crates/workspace-manager/src/lib.rs`.
- [x] T010 Implement `SharedRepositoryStore::ensure` in `crates/workspace-manager/src/shared_repository.rs`: E1 early-out, **E2 clone outside the lease** into `.{repo_id}.incoming` (constitution XII — no lock across an awaited clone), E3 `rename(2)` publication, E4 gc config before any worktree is added, E5 setgid + in-process umask before the first object, E6 remotes copied with `origin` retargeted, E7 `cat-file -e` proof of the target branch, E8 best-effort branch push-back. (depends on T008, T009)
- [ ] T011 Tests in `crates/workspace-manager/src/shared_repository.rs`: `ensure` is idempotent; an interrupted clone leaves no valid store; remotes copied and `origin` retargeted; gc config present before the first worktree; a missing target branch fails; a directory with no refs is rejected; **two concurrent `ensure` calls for one repository produce one store** (constitution XII's both-orderings requirement, and the dedup C1 actually relies on). (depends on T010)

## Phase 4: Administration and resolver — landed together

> Phases 3 and 5 of the first draft were split; between them, new cluster
> branches would exist only in the store while every route still read
> `repo.path`, regressing branch status, diff and PR. Constitution III wants
> shippable steps, so they land as one.

- [x] T012 Add `git_path: PathBuf` to `RepoWorkspaceInput` in `crates/workspace-manager/src/workspace_manager.rs`; `::new` stays behaviour-preserving, `::shared` supplies the store. Add `WorkspaceError::WorktreeNotPortable`.
- [x] T013 Replace `&repo.path` with the resolved git path in `crates/workspace-manager/src/workspace_manager.rs` at `:213` (`check_branch_exists` in `add_repository`), `:255` (`prepare_deletion_context`), `:420`, `:430` (the non-fenced arm), **`:444` (`RepoWorktree::source_repo_path` — required; the first draft wrongly claimed no edit was needed)**, `:540`, `:546`, `:554`, `:571`, `:581`, `:670`, `:676`. (depends on T012)
- [x] T014 Change `WorkspaceManager::cleanup_workspace` to take resolved inputs rather than `&[Repo]`; update the caller at `crates/local-deployment/src/container.rs:911`. (depends on T012)
- [x] T015 Add the single resolver `workspace_repo_git_path(workspace, repo)` in `crates/local-deployment/src/container.rs`. Matches `WorkspacePlacementState` **exhaustively, with no wildcard arm**, over all six variants: `Local` → `repo.path`; `Reserved`/`Provisioning`/`Ready`/`Failed`/**`Cleaning`** → the store. Returns `repo.path` whenever clustering is disabled. Reads the persisted placement row. (depends on T009)
- [x] T016 In `create_cluster_workspace` (`crates/local-deployment/src/container.rs:3626`) call `ensure` per repo before `create_workspace_fenced` and pass `RepoWorkspaceInput::shared`; on failure transition `Provisioning → Failed` with a reason naming the repo. Acquire and fully release the admin lease around each critical section — never nested. (depends on T010, T013)
- [x] T017 Assert portability for every worktree after creation and before the `Ready` transition, in `crates/local-deployment/src/container.rs`. A violation is a `Failed` placement. (depends on T002, T016)
- [ ] T018 Convert the branch-scoped call sites in `crates/server/src/routes/workspaces/git.rs:207,227,441,445,452,470,520,536,605,720,750`. (depends on T015)
- [ ] T019 Convert the branch-scoped call sites in `crates/server/src/routes/workspaces/pr.rs:204,418,439,574,**589**,706,709`. `:589` (`get_pr_comments`) was missing from the first draft. (depends on T015)
- [ ] T020 Convert `get_base_commit` and `DiffStreamArgs::repo_path` at `crates/local-deployment/src/container.rs:3367,3386`. (depends on T015)
- [ ] T021 [P] Record why these stay on `repo.path`, in a comment at each site: `crates/server/src/routes/repo.rs:92,105,167,223,260,263` and `crates/local-deployment/src/container.rs:2060` (`copy_project_files` — a bare store has no working tree).
- [ ] T022 Audit the second child-process boundary for Git path assumptions: `crates/local-deployment/src/pty.rs` (the real home of `PtyService`), `crates/services/src/services/container.rs` and `crates/server/src/routes/terminal.rs`. (depends on T015)
- [ ] T023 Tests in `crates/local-deployment/src/container.rs`: the resolver returns `repo.path` for `Local` and the store for `Ready`; **and `repo.path` for every state when clustering is disabled** (acceptance criterion 9, which the first draft left untested); a non-portable repo yields `Failed` with a reason and no `Ready` workspace; a `local` workspace still routes through `ensure_workspace_exists` with `repo.path` — the regression PR #172 called out as untested. (depends on T017, T020)

## Phase 5: Adoption of the broken worktrees (contract 2, `adopt`)

- [ ] T024 Implement `SharedRepositoryStore::adopt(repo, worktree_path, branch)` and `AdoptOutcome` in `crates/workspace-manager/src/shared_repository.rs`. The signature takes `repo` because the store is underivable from a worktree whose pointer names `/srv/src`. Guarantees A1 (no working-tree mutation), A2 (same-directory temp + `rename(2)`; never unlink `.git`), A5 (fail before mutating), A6 (idempotent no-op), A7 (re-probe after `git worktree repair`), A8 (`info!` path + reason + action **before** acting), A9 (no DB writes). (depends on T002, T010)
- [x] T025 Implement the refusal set (A3, A4) in `crates/workspace-manager/src/shared_repository.rs`: unparseable `.git`; real `.git` directory; branch not provable with `cat-file -e`; branch already checked out by another worktree of the store. Never fall back to the target branch. (depends on T024)
- [ ] T026 Wire lazy adoption into `ensure_container_exists` (`crates/local-deployment/src/container.rs:2600`) for cluster-placed workspaces. (depends on T025)
- [ ] T027 Allow a cluster workspace left `Failed` by the portability assertion to re-enter provisioning when adoption succeeds, in `crates/local-deployment/src/container.rs`. Without this, `ensure_container_exists` (`:2632-2637`) rejects every non-`Ready` state and a failed workspace is permanently unrecoverable. (depends on T026)
- [ ] T028 Add the bounded coordinator-boot sweep in `crates/local-deployment/src/container.rs`. **One sweep, one enumeration source** — the database's cluster-placed workspaces (the first draft had two sweeps with divergent sources). Concurrency-limited; per-workspace failure isolation; skips workspaces whose assigned worker is unreachable **as judged by the lease/heartbeat evidence channel, never a health or metrics probe** (constitution XIX); truthful aggregate; never writes `container_ref`. (depends on T025)
- [x] T029 Tests in `crates/workspace-manager/src/shared_repository.rs`, all on fixture roots: hide the main repo, clone a store, adopt — assert branch, `git log`, tracked content, untracked survival and a clean index; healthy worktree untouched; `.git` directory untouched; unresolvable branch refuses; branch checked out elsewhere refuses; adoption is idempotent. (depends on T025)

## Phase 6: Enforcement, worker preflight, retention

- [ ] T030 Run the portability probe over all cluster-placed workspaces at boot, at placement, and at dispatch, in `crates/local-deployment/src/container.rs`. Enumerate every violation in one pass with an actionable remedy; do not abort on the first. Reports through structured logging and the placement reason — **not** through `vk-deploy-notify`, which is a systemd/Nix helper with no Rust API. (depends on T017, T028)
- [x] T031 Make `discover_repo_names` (`crates/worker/src/execution.rs:479-494`) distinguish "no repositories" from "could not enumerate". Today a read failure yields an empty list, so preflight would probe nothing and the dispatch would be accepted — defeating the probe one frame above it. Also replace its `.exists()` with `try_exists()`.
- [x] T032 Add worker preflight in `crates/worker/src/execution.rs` after `authorize_workspace_path` (`:173-180`), probing each directory from `discover_repo_names`. Reject with a typed reason and terminalise the worker job; refuse on "could not enumerate". Never repair (FR-23); never accept a same-named local directory — the live case, since `/srv/src/homelab` exists on workers with no `worktrees/`. Add no dependency to `crates/worker/Cargo.toml`. (depends on T002, T031)
- [ ] T033 Implement store retention (FR-29) in `crates/workspace-manager/src/workspace_manager.rs` / the reclamation path: retain while any workspace references the repository; never remove as a side effect of deleting one workspace; any error retains. Untasked in the first draft, and a constitution XV violation while the sibling reclamation routines disagree. (depends on T009)
- [ ] T034 Tests: dispatch into a workspace with a dangling `.git` is rejected and the worker job reaches a terminal state, in `crates/worker/src/execution.rs`; a worker never substitutes a same-named local repository (FR-23). (depends on T032)
- [ ] T035 [P] Test store retention in `crates/workspace-manager/src/shared_repository.rs`: referenced store retained; unreferenced-and-repo-deleted store reclaimed; an error during the determination retains. (depends on T033)

## Phase 7: Verification

- [x] T036 `cargo check --workspace --exclude vibe-kanban-tauri --all-targets`.
- [x] T037 `cargo clippy --workspace --exclude vibe-kanban-tauri --all-targets -- -D warnings`.
- [x] T038 `cargo test --workspace --exclude vibe-kanban-tauri`.
- [ ] T039 **Cannot run in this environment**: `pnpm run check`, `pnpm run lint`, `pnpm run format`. This worker has no `pnpm` or `npm` and no network-installed Node package manager was added. The change is backend-only, so `shared/types.ts` should be untouched — assert that by inspection and `cargo fmt` the Rust. Report the gap in the completion notes rather than claiming the gates passed (constitution XIV).
- [ ] T040 Independent Codex review of the diff; iterate until it reports no significant findings.
- [ ] T041 Two-node deployment gate: create a workspace on a worker, run an agent turn that commits, open a PR from the coordinator, **disconnect the coordinator, cancel a process group**, remove the shared mount, verify worktree integrity. Passing unit tests does not replace this.

## Phase 8: Documentation and knowledge

- [ ] T042 [P] Update `docs/self-hosting/clustered-workers.mdx`: `repositories/` is created and owned by Vibe Kanban rather than by the operator, and a repository registered outside the shared mount is supported rather than silently broken.
- [ ] T043 [P] Extend `docs/knowledge-base/clustered-workspace-execution.md` — its "Keep shared Git administration single-writer" section is wrong in one respect (workers *do* write into the store) and incomplete in another (the portability invariant). Tag `19a4-git-worktrees-br`.
- [ ] T044 Refresh `docs/knowledge-base/INDEX.md`. (depends on T043)

<!--
Conventions:
- `T001` … task ids are stable and referenced by the dependency graph.
- `[P]` … parallel-safe (no shared file with a neighbouring task).
- `[ ]` / `[x]` … completion checkbox.
-->
