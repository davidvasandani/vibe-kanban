# Implementation Plan — `19a4-git-worktrees-br`

Repair Git worktrees in cluster mode. Design: [`SPEC.md`](SPEC.md). Constraints:
[`PRIOR_KNOWLEDGE.md`](PRIOR_KNOWLEDGE.md).

## Before you start

This workspace runs on a **worker** node (`think-cluster`), which has no package
manager and, by default, no Rust or pnpm. A toolchain has been bootstrapped:

```bash
source /srv/vibe-kanban-shared/cluster/workspaces/19a4a176-a50a-4bc8-8f68-607547fd2516/TOOLCHAIN_ENV.sh
cargo check --workspace --exclude vibe-kanban-tauri --all-targets
```

`vibe-kanban-tauri` is excluded because webkit2gtk is not in `/nix/store` here;
CI excludes it too. `CARGO_TARGET_DIR` points at `/tmp` — never put a target
directory on the NFS share.

**Git does not work in this worktree.** That is the bug. Edit files; the
coordinator owns the commit.

**Two things must not happen during this task** (both from `PRIOR_KNOWLEDGE.md`
§D):

- Do not start a dev server on this host. Not because of the orphan sweep —
  `cleanup_orphan_workspaces` is **inert** while clustering is enabled
  (`workspace_manager.rs:714-719` early-returns when `allow_reclamation == false`,
  and `container.rs:1025` passes `!cluster_config.enabled`) — but because a
  second server would contend for the same shared root, the same SQLite
  repository administration leases, and the same live worktrees this task exists
  to repair.
- Do not run any repair against the live `{shared_root}/workspaces` tree. All
  behaviour is exercised against temporary fixture roots.

---

## Layer 0 — Baseline

1. Source `TOOLCHAIN_ENV.sh` and record a green
   `cargo check --workspace --exclude vibe-kanban-tauri --all-targets` so later
   failures are attributable. Expect `ci/check-project-context.sh` to be red for
   unrelated reasons (`PRIOR_KNOWLEDGE.md` #56).
2. Read the three call-site clusters end to end before editing:
   `crates/workspace-manager/src/workspace_manager.rs`,
   `crates/worktree-manager/src/worktree_manager.rs`, and the `repo.path`
   consumers listed in `SPEC.md` §3.

---

## Layer 1 — The portability probe (no behaviour change)

Pure, dependency-free, testable in isolation. Nothing else can be trusted until
this exists.

3. New `crates/utils/src/worktree_linkage.rs` — **`utils`, not `git`**.
   `crates/worker` must call this and depends on `utils`, not on `git`; putting
   it in `crates/git` would drag `git2` into the worker. `crates/git` re-exports
   nothing and calls it directly. Type `WorktreeLinkage` with:
   - `probe(worktree_path) -> Result<WorktreeLinkage, LinkageError>` —
     **pure filesystem, no subprocess.** Reads `.git`, parses the `gitdir:` line,
     resolves `commondir`, and follows the store's `worktrees/<n>/gitdir` back.
     Uses `try_exists()` throughout; never `read_dir(..).filter_map(|e| e.ok())`.
     This is the half the worker runs.
   - A status enum with **distinct** variants: `Portable`, `NotApplicable`
     (local placement / plain `.git` directory), `Dangling { target }`,
     `OutsideSharedRoot { common_dir }`, `Indeterminate { reason }`. A failed
     read is never reported as `Portable`, and `NotApplicable` is never reported
     as a failure.
   - `assert_common_dir_within(shared_root)` — a structural assertion, not a
     substring match on `/srv/src`. Pure; available everywhere.
   - `assert_toplevel_is(path)` — **coordinator-only**, and the only part that
     may spawn `git rev-parse --show-toplevel`. Gate it so the worker path never
     reaches it. The ancestor-repository hazard is already covered on the pure
     path: a `.git` *file* naming a resolvable `worktrees/<n>` whose `gitdir`
     points back to this exact worktree cannot belong to an ancestor, because an
     ancestor's registration would name the ancestor's path. This assertion is
     defence in depth where a subprocess is affordable.
   - Every filesystem/subprocess probe bounded by a timeout (NFS hangs).
4. `[P]` Unit tests over fixture roots: `.git` file → valid; `.git` file →
   missing target; `.git` file → target outside root; real `.git` directory; no
   `.git` at all; symlink escape; and a worktree nested under an ancestor
   repository (must not report `Portable`, on the pure path, with no subprocess).
5. `[P]` Add the changed crates to the `backend` path filter in
   `.github/workflows/test.yml`. `crates/git/**` is **already** there (`:64`);
   the genuinely missing ones are `crates/workspace-manager/**`,
   `crates/worktree-manager/**`, `crates/worker/**` and
   `crates/cluster-protocol/**`, which hold the store, the namespace containment
   and the worker preflight. A test command in a job the changed files do not
   trigger is not coverage.

---

## Layer 2 — The shared repository store

6. New `crates/workspace-manager/src/shared_repository.rs`,
   `SharedRepositoryStore::new(shared_root, locks)` — it takes the
   `RepositoryAdminLockManager`, because every one of its operations is fenced
   and the manager is not reachable from a bare `shared_root`.
   - `path_for(repo_id)` delegates to `SharedWorkspacePaths::repository_dir`
     (today dead code) so there is exactly one canonical form. Canonicalise once,
     at this boundary.
7. `ensure(repo, target_branch) -> Result<PathBuf>`. **Clone outside the lease,
   publish inside it** — constitution XII forbids holding a coordination lock
   across an awaited external operation, and a multi-minute clone of a large
   repository could outlive the bounded SQLite lease, silently unfencing the
   very operation the lease exists to fence:
   - a. Early-out, **unlocked**, when the store already exists and resolves
        `target_branch` (`cat-file -e <sha>^{commit}`). This runs on every
        provisioning and must be cheap.
   - b. **Outside the lease**: create `repositories/.{repo_id}.incoming` setgid,
        group-owned, umask `002` — **before** the first object is written — and
        `git clone --bare <repo.path>` into it. A per-repo staging directory,
        never a shared one.
   - c. Acquire `RepositoryAdminLockManager::acquire(repo.id, …)`. Everything
        from here to (h) is short and bounded.
   - d. Re-check the early-out under the lease. A concurrent `ensure` for the
        same repository may have published while this one cloned; the loser
        observes the winner's store here and discards its staging directory.
        **This is the dedup mechanism** — there is no cross-crate mutex to lean
        on, and none is minted.
   - e. Verify the staged clone: real refs, resolvable HEAD, target branch proven
        with `cat-file -e`. Assert the closure, not the rollup.
   - f. Copy every remote name/URL from `repo.path`, replacing the `origin` that
        `clone --bare` points at the local checkout. A clone-of-a-clone's
        `origin/main` means the wrong thing. Set `gc.auto=0`,
        `gc.autoDetach=false`, `maintenance.auto=false`,
        `gc.worktreePruneExpire=never`, `core.logAllRefUpdates=true`.
   - g. `rename(2)` the staging directory into place. A partially created store
        is never observable as valid.
   - h. Fetch `+refs/heads/*:refs/heads/*` from `repo.path`, and `target_branch`
        from the real remote when one exists. Per-remote failure is tolerated;
        the target branch resolving afterwards is **not** optional.
   - i. Release the lease before returning. `ensure`, `adopt` and
        `create_workspace_fenced` each acquire and **fully release** the lease
        around their own critical section, and none is called while another holds
        it: `acquire` (`worktree_manager.rs:117-151`) takes an owned
        `tokio::sync::Mutex` guard and *then* an exclusive SQLite lease, so a
        nested acquire deadlocks. Idempotent and re-runnable.
8. `[P]` Tests: `ensure` is idempotent; an interrupted clone leaves no valid
   store; remotes are copied and `origin` retargeted; gc config present; a
   missing target branch fails rather than proceeding; a created directory with
   no refs is rejected (assert the closure, not the rollup); and — the
   constitution XII regression — two concurrent `ensure` calls for one repository
   produce one store, one clone, and no deadlock.

---

## Layer 3 — Route worktree administration at the store

9. Add `git_path: PathBuf` to `RepoWorkspaceInput`; keep
   `RepoWorkspaceInput::new` behaviour-preserving (`git_path = repo.path`) and
   add `::shared(repo, target_branch, store)`.
10. In `workspace_manager.rs`, replace `&repo.path` with `&input.git_path` at
    `:420`, `:430` (the non-fenced `create_worktree` arm — it reads
    `&input.repo.path`, which is easy to miss when grepping for `&repo.path`),
    `:540`, `:546`, `:554`, `:571`, `:581`, `:670`, `:676`, plus three sites that
    do not feed `create_worktree` and were missed earlier:
    - `:213` — `check_branch_exists(&repo.path, …)` in `add_repository`;
      attaching a repo to an existing cluster workspace otherwise validates the
      branch in the wrong store.
    - `:255` — `repo_paths` in `prepare_deletion_context`.
    - `:444` — `source_repo_path: input.repo.path.clone()`. **This one is
      required**, contrary to the earlier claim that `RepoWorktree` needs no
      edit: it reads `input.repo.path`, not `input.git_path`, so without the
      change every cluster workspace carries the coordinator-local store into
      rollback and cleanup. Once changed, `source_repo_path` propagates correctly
      to `:695`.

    Also convert `crates/server/src/routes/workspaces/pr.rs:589`
    (`get_pr_comments`). Leave `crates/local-deployment/src/container.rs:2060`
    (`copy_project_files`) on `repo.path` and say why in a comment: it copies out
    of the registered checkout's *working tree*, and a bare store has none.
11. Change `WorkspaceManager::cleanup_workspace` to take the resolved inputs
    rather than `&[Repo]`; update the caller at
    `crates/local-deployment/src/container.rs:911`.
12. `create_cluster_workspace` (`container.rs:3626`) calls
    `SharedRepositoryStore::ensure` per repo before `create_workspace_fenced` and
    passes `RepoWorkspaceInput::shared`. On `ensure` failure the placement
    transitions to `Failed` with a reason naming the repo — never `Ready`.
13. After creation and **before** the `Ready` transition, assert P1 for every
    worktree using Layer 1. A violation is a `Failed` placement.
14. `[P]` Tests: cluster provisioning uses the store; a non-portable repo yields
    `Failed` with a reason and no `Ready` workspace; a `local` workspace still
    routes through `ensure_workspace_exists` with `repo.path` (the regression
    #172 called out as untested).

---

## Layer 4 — Contain the shared `worktrees/` namespace

Do this before anything touches the live fleet: with one store per repo, these
are cluster-wide, not per-node.

15. `force_cleanup_worktree_metadata` (`worktree_manager.rs:755-777` — `:718-748`
    is the retry block, not the cleanup) is **already** path-resolved: it goes
    through `find_worktree_git_internal_name` (`:573-610`), which reads every
    `worktrees/*/gitdir` and compares canonicalised paths. Keep that resolution;
    constitution VI says do not rebuild it. Fix the two error-swallowing idioms
    inside it instead: `read_dir(...).filter_map(|entry| entry.ok())` (`:583-585`)
    and `gitdir_path.exists()` (`:599`) both turn *indeterminate* into *absent*,
    so a transient NFS read failure returns `Ok(None)` and the caller falls
    through to a broader cleanup against a namespace that now holds every
    workspace of the repo. Use `try_exists()` and a propagating `read_dir`, and
    audit `comprehensive_worktree_cleanup` on the same terms.
16. Scope `comprehensive_worktree_cleanup`'s trailing repo-wide
    `git worktree prune` to the worktree being cleaned. A known-unfixed
    pre-existing defect, in scope here because the blast radius changes.
17. `[P]` Test: two workspaces of the same repo share one store; cleaning up one
    leaves the other's registration *and* directory intact. This is the test that
    would have caught the 2026-07-05 prune incident.

---

## Layer 5 — Route workspace-scoped Git operations

18. Add one resolver — `workspace_repo_git_path(&self, workspace, repo)` —
    returning the store for worker placements and `repo.path` for `local`.
    `WorkspacePlacementState` has **six** variants
    (`crates/db/src/models/workspace.rs:105-113`): `Local`, `Reserved`,
    `Provisioning`, `Ready`, `Failed`, `Cleaning`. The worker-placement set is
    `Reserved`/`Provisioning`/`Ready`/`Failed`/**`Cleaning`** — `Cleaning`
    included, because cleanup of a cluster workspace must run against the store
    it was created from; falling through to `repo.path` there is the FR-24
    failure. Match the enum **exhaustively, with no wildcard arm**, so a future
    variant is a compile error rather than a silent fallthrough. It reads the
    persisted placement row, never the request host. Exactly one resolver; no
    duplicated cluster branching in routes.
19. Convert the call sites listed in `SPEC.md` §3:
    `routes/workspaces/git.rs:207,227,441,445,452,470,520,536,605,720,750`;
    `routes/workspaces/pr.rs:204,418,439,574,589,706,709`;
    `container.rs:3367` and `:3386`.
20. Leave `routes/repo.rs:92,105,167,223,260,263` and
    `container.rs:2060` (`copy_project_files`) on `repo.path` and say why in a
    comment — a bare store has no working tree, and these describe the registered
    repository rather than the workspace branch.
21. Audit the second child-process boundary (`PtyService`,
    `crates/local-deployment/src/pty.rs`, and `routes/terminal.rs`) for any Git
    path assumption. Fixing only the executor path leaves interactive terminals
    wrong.
22. `[P]` Tests: the resolver returns `repo.path` for `local` and the store for
    `ready`; branch status and diff base resolve against the store for a cluster
    workspace.

---

## Layer 6 — Adoption of the broken worktrees

Measured scope: 15 worktrees across 9 cluster workspaces — every cluster-placed
workspace on the share, 100% broken, 0 resolving.

23. `SharedRepositoryStore::adopt(&self, repo: &Repo, worktree_path: &Path,
    branch: &str)` implementing `SPEC.md` §5 steps 1–7. It takes `&Repo` because
    the store is not derivable from the worktree: a broken worktree's pointer
    names `/srv/src/<repo>`, which identifies no store, and adoption also needs
    the repo to read the old tip from and to push back to. It acquires and fully
    releases the administration lease around its own critical section, and is
    called only *after* `ensure` has returned and released — never nested.
    Non-destructive: pointer files only, same-directory temp + `rename(2)`, and
    the `.git` marker is never transiently unlinked.
24. Refuse to adopt when: the worktree is already portable (early-out); `.git` is
    a real directory; the branch cannot be resolved and proven with
    `cat-file -e`; or the branch is already checked out by another worktree of
    the store.
25. Wire adoption into `ensure_container_exists` for cluster-placed workspaces,
    and into a bounded coordinator-boot sweep over `{shared_root}/workspaces` —
    which naturally excludes the deploy build tree. Skip (do not repair)
    workspaces whose assigned worker is unreachable: unreachable is
    indeterminate, not idle.
26. Per-worktree best-effort with a truthful aggregate; `info!` the path, the
    reason it was selected, and the action **before** acting; never return
    `Ok(())` for a repair that silently failed. No database writes beyond the
    placement reason already owned.
27. Adoption step 7 — push the branch back into `repo.path` — for rollback
    safety, so a pre-fix binary still resolves the branch and reattaches
    non-destructively instead of recreating.
28. `[P]` Tests, all on fixture roots: the reproduction from `SPEC.md` (hide the
    main repo, clone a store, adopt) asserting branch, log, tracked content,
    untracked survival and a clean index; healthy worktree untouched; `.git`
    directory untouched; unresolvable branch refuses; branch checked out
    elsewhere refuses; adoption is idempotent.

---

## Layer 7 — Level-triggered enforcement and worker preflight

29. Run the Layer 1 probe over all cluster-placed workspaces on coordinator boot,
    at placement, and at dispatch, with per-workspace failure isolation.
    Enumerate *every* non-portable worktree in one pass with a concrete repair
    action; do not abort on the first. A creation-time-only check is an edge
    trigger and stalls silently.
30. Worker side (`crates/worker/src/execution.rs`), after
    `authorize_workspace_path`: probe each discovered repo directory
    (`discover_repo_names`, `:479-494`) and reject the dispatch with a typed
    reason when the linkage does not resolve. Terminalise the worker job — a
    failed dispatch left pending contaminates reconciliation. Never fall back to
    an identically named local directory. The worker calls `probe()` only — the
    pure-filesystem half, from `crates/utils`, which it already depends on —
    never `assert_toplevel_is`. No `git`/`git2` dependency is added to the worker
    crate.
31. Report the "N worktrees failed the portability invariant" aggregate through
    **structured logging and the placement reason** — both in-process and already
    surfaced. Do not wire it to `vk-deploy-notify`: that is a systemd/Nix helper
    in `homelab/modules/vibe-kanban-rebuild.nix`, not a Rust API — it appears
    nowhere in `crates/` — and that module is out of scope. External notification
    is a possible follow-up, not part of this change.
32. `[P]` Test: dispatch into a workspace with a dangling `.git` is rejected and
    the worker job reaches a terminal state.

---

## Layer 8 — Verification

Be exact about which gates run here. This workspace is on a worker node with
**no pnpm and no npm**, and no package manager to install them, so three of the
repository's standard commands are unrunnable. Constitution XIV forbids silently
skipping a toolchain after reporting success, so they are named rather than
listed as if they will pass.

**Runs here** (after `source TOOLCHAIN_ENV.sh`):

33. `cargo check --workspace --exclude vibe-kanban-tauri --all-targets`.
34. `cargo clippy --workspace --exclude vibe-kanban-tauri --all-targets -- -D warnings`.
35. `cargo test --workspace --exclude vibe-kanban-tauri`.

**Cannot run here** — record as not-run, with the reason, in the completion
notes; CI has the toolchain and is the authority for them:

36. `pnpm run check`, `pnpm run lint`, `pnpm run format` — no pnpm/npm on this
    host. `cargo fmt` covers the Rust half of `format` and *is* run. The change
    is backend-only, so no generated TypeScript moves; confirm `shared/types.ts`
    is unchanged by inspection rather than by running `generate-types`.
37. Independent Codex review of the diff; iterate until it reports no significant
    findings.

---

## Layer 9 — Documentation and knowledge

38. Update `docs/self-hosting/clustered-workers.mdx`: the `repositories`
    directory is now created and owned by Vibe Kanban rather than by the
    operator, and a repo registered outside the shared mount is supported (its
    store is provisioned automatically) rather than silently broken.
39. Record the two-node deployment gate result. Passing unit tests does not
    replace it.
40. Extend `docs/knowledge-base/clustered-workspace-execution.md` — its "Keep
    shared Git administration single-writer" section is now wrong in one respect
    (workers *do* write into the store) and incomplete in another (the
    portability invariant). Tag with `19a4-git-worktrees-br` and refresh
    `docs/knowledge-base/INDEX.md`.

---

## Not in this plan

`crates/cluster-protocol`, the heartbeat, the scheduler, `worker_nodes`, preview
routing, and `homelab/modules/vibe-kanban-rebuild.nix` are untouched. The shared
root, mount and NFS export are already correct; `/srv/src` stays local
build-input storage.
