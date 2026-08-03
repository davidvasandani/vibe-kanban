# Implementation Plan: Portable Git worktrees for cluster-placed workspaces

**Spec**: `./spec.md`
**Clarifications**: `./clarifications.md`
**Status**: Draft

The full engineering design lives at the repository root in
[`SPEC.md`](../../../SPEC.md), with the layered work breakdown in
[`IMPLEMENTATION_PLAN.md`](../../../IMPLEMENTATION_PLAN.md) and the distilled
knowledge-base constraints in [`PRIOR_KNOWLEDGE.md`](../../../PRIOR_KNOWLEDGE.md).
This document is the SpecKit-shaped view: how the functional requirements map
onto real files, and where the constitution binds.

## Technical Context

- **Language**: Rust (nightly-2025-12-04, `rust-toolchain.toml`), Tokio/Axum.
  No frontend change.
- **Storage**: coordinator-local SQLite (`crates/db`) plus one shared NFSv3
  export, `172.16.0.99:/var/nfs/shared/VibeKanban`, mounted at
  `/srv/vibe-kanban-shared` on every node with `VK_CLUSTER_SHARED_ROOT =
  /srv/vibe-kanban-shared/cluster`.
- **Topology**: coordinator `think2`; workers `think3`, `think4`. Workers are
  hosts, not containers, and mount *only* the shared export. `/srv/src` is
  per-host local storage.
- **The defect**: `git -C <repos.path> worktree add <shared_path>` records
  absolute paths in both directions. `repos.path` is `/srv/src/<repo>`, invisible
  to workers, so `{workspace}/{repo}/.git` dangles.
  (`crates/workspace-manager/src/workspace_manager.rs:420` →
  `crates/worktree-manager/src/worktree_manager.rs:705` →
  `crates/git/src/cli.rs:84`.)
- **Constraint**: the worker crate has no `git`, `db`, `worktree-manager` or
  `workspace-manager` dependency (`crates/worker/Cargo.toml`), and must not gain
  one. It *does* depend on `utils`, which is why the probe lands there. Worker-
  side work is filesystem reads only.
- **Constraint**: `/srv/src/<repo>` is `forceSync = true` and hard-reset every
  15 minutes by `git-projects-update`; it is declared build-input-only by
  `homelab/modules/vibe-kanban-rebuild.nix:10-13`. Workspace branches cannot live
  only there.
- **Verification environment**: this workspace is on a worker with no package
  manager — in particular **no pnpm and no npm**. A pinned toolchain is
  bootstrapped by `TOOLCHAIN_ENV.sh` at the workspace root; baseline
  `cargo check --workspace --exclude vibe-kanban-tauri --all-targets` is green.
  `cargo check`, `cargo clippy -- -D warnings`, `cargo test` and `cargo fmt` run
  here; `pnpm run check`, `pnpm run lint` and `pnpm run format` cannot, and are
  reported as not-run with that reason rather than skipped silently
  (constitution XIV). The change is backend-only, so no generated TypeScript
  moves; `shared/types.ts` is checked by inspection.

## Architecture & Approach

Vibe Kanban takes ownership of a **bare repository store per repo** at
`{shared_root}/repositories/{repo_id}` — the path
`SharedWorkspacePaths::repository_dir` already computes and which nothing
currently uses (`crates/workspace-manager/src/workspace_manager.rs:53-55`,
referenced only by `create_base_dirs` and unit tests). Cluster worktrees are
created from that store, so every recorded path resolves identically on every
node.

| FR | Where it lands |
| --- | --- |
| FR-1, FR-3, FR-4, FR-5 | New `WorktreeLinkage` probe in **`crates/utils/src/worktree_linkage.rs`** (not `crates/git` — `crates/worker` depends on `utils` and must not gain `git`/`git2`). `probe()` is pure filesystem: reads `.git`, resolves `gitdir:` and `commondir`, follows the store's `worktrees/<n>/gitdir` back. `assert_common_dir_within(shared_root)` is a structural containment check, never a substring test. `assert_toplevel_is` (`--show-toplevel`) is a coordinator-only extra that may spawn `git`; the ancestor-repository hazard is already closed on the pure path by the two-sided pointer check. |
| FR-2, FR-6, FR-8, FR-9 | New `crates/workspace-manager/src/shared_repository.rs`, `SharedRepositoryStore::new(shared_root, locks)`. `ensure()` clones into `repositories/.{repo_id}.incoming` **outside** the admin lease, then takes `RepositoryAdminLockManager::acquire` (`worktree_manager.rs:117-151`) only for re-check → verify → configure → `rename(2)` → fetch, and releases it before returning. Remotes are copied from `repo.path`, replacing the `origin` that `clone --bare` points at the local checkout. |
| FR-7 | `create_cluster_workspace` (`crates/local-deployment/src/container.rs:3626`) transitions `Provisioning → Failed` with a reason naming the repo; the existing `WorkspacePlacement::transition` call at `:3689` already carries it. |
| FR-10 | `git cat-file -e <sha>^{commit}` — `git rev-parse` echoes any well-formed hex string and proves nothing. |
| FR-11 … FR-18 | `SharedRepositoryStore::adopt(&self, repo, worktree_path, branch)` — it takes `&Repo` because a broken worktree's pointer names `/srv/src/<repo>` and identifies no store, and adoption needs the repo to read the old tip from and to push back to. Pointer files only, same-directory temp + `rename(2)`, `git worktree repair`, `git reset` to rebuild the index, re-probe. The `.git` marker is never transiently unlinked. |
| FR-17 | "Unreachable" is read from the lease and heartbeat records that already govern dispatch — never a health endpoint or a metrics surface (constitution XIX). |
| FR-19, FR-20, FR-21 | The probe runs at coordinator boot (bounded sweep over `{shared_root}/workspaces`), at placement, and at dispatch. `NotApplicable` is a distinct variant from `Broken`. |
| FR-22, FR-23 | `crates/worker/src/execution.rs`, after `authorize_workspace_path` (`:173-180`) and alongside `discover_repo_names` (`:479-494`). It calls `probe()` only — the pure half, from `utils`. Refuse and terminalise; never repair, never accept a same-named local directory. |
| FR-24, FR-25 | `force_cleanup_worktree_metadata` (`worktree_manager.rs:755-777`; `:718-748` is the retry block) is *already* path-resolved via `find_worktree_git_internal_name` (`:573-610`) — keep it. Fix instead the two error-swallowing idioms inside it, `read_dir(...).filter_map(|entry| entry.ok())` (`:583-585`) and `gitdir_path.exists()` (`:599`), which make an NFS read failure return `Ok(None)` so the caller falls through to a broader cleanup against a namespace that now holds every workspace of the repo. Scope `comprehensive_worktree_cleanup`'s trailing repo-wide `git worktree prune`; set `gc.auto=0`, `gc.autoDetach=false`, `maintenance.auto=false`, `gc.worktreePruneExpire=never` on the store. |
| FR-26, FR-27 | One resolver, `workspace_repo_git_path(workspace, repo)`, returning `repo.path` for `local` placement and when clustering is disabled, and the store for `reserved`/`provisioning`/`ready`/`failed`/**`cleaning`** — all six variants of `WorkspacePlacementState` (`crates/db/src/models/workspace.rs:105-113`), matched exhaustively with no wildcard arm so a new variant is a compile error. `cleaning` belongs with the worker placements: cleanup must administer worktrees in the store they were created from. This is exactly the distinction PR #172 established and must not be re-broken. |
| FR-28 | Best-effort push of the workspace branch back into `repo.path` on every `ensure`. |
| FR-29 | Retention wired into the existing reclamation sweep; retain on any error. |

The plumbing change is deliberately narrow: `RepoWorkspaceInput` gains a
`git_path` field, and inside `workspace_manager.rs` every `&repo.path` that feeds
worktree administration becomes `&input.git_path` — `:420`, `:430` (the
non-fenced `create_worktree` arm, which reads `&input.repo.path` and so does not
match a grep for `&repo.path`), `:540`, `:546`, `:554`, `:571`, `:581`, `:670`,
`:676` — plus three sites that do not feed `create_worktree`: `:213`
(`check_branch_exists` in `add_repository`, otherwise validating the branch in
the wrong store when a repo is attached to an existing cluster workspace), `:255`
(`repo_paths` in `prepare_deletion_context`), and `:444`.

`:444` is **required**, contrary to an earlier claim that `RepoWorktree` needs no
edit: it constructs `source_repo_path: input.repo.path.clone()`, not
`input.git_path`, so without the change every cluster workspace carries the
coordinator-local store into rollback and cleanup. Once changed, the value
propagates correctly to `:695`.

Server-side, the resolver replaces `&repo.path` at
`crates/server/src/routes/workspaces/git.rs:207,227,441,445,452,470,520,536,605,720,750`,
`crates/server/src/routes/workspaces/pr.rs:204,418,439,574,589,706,709` (`:589`
is `get_pr_comments`), and
`crates/local-deployment/src/container.rs:3367,3386`. It is deliberately **not**
applied at `crates/server/src/routes/repo.rs:92,105,167,223,260,263` or at
`crates/local-deployment/src/container.rs:2060` (`copy_project_files`) — a bare
store has no working tree, and those describe the registered repository rather
than the workspace branch. `copy_project_files` copies *out of* that working
tree, so routing it at the store would break `copy_files` outright.

## Data Model

See [`./data-model.md`](./data-model.md). No schema migration: the change is
entirely about filesystem layout and path resolution over existing tables
(`repos`, `workspaces`, `workspace_placements`, `repository_admin_locks`).

## Contracts

See [`./contracts/`](./contracts/). No wire-protocol change —
`crates/cluster-protocol` is untouched. The contracts captured are the internal
seams: the linkage probe, the store, and the resolver.

## Research Notes

See [`./research.md`](./research.md) for the rejected alternatives (exporting
`/srv/src`, per-workspace clones, `objects/info/alternates`, rewriting
`repos.path`, documentation-only), the two measurements taken on the production
export, and the note that **no new dependency is introduced**.

## Constitution Check

Checked against `.specify/memory/constitution.md` v0.18.0.

All twenty principles are checked. Seven were checked in an earlier draft; the
three that bind hardest and were missing — II, XII and XIX — are called out
below, and the XI row is restated (XI is about preserving exact backend text in
the UI; the typed-absence property this feature relies on is XIX).

| Principle | Status |
| --- | --- |
| I. Clarity over cleverness | Honoured. One resolver, one probe, one store type; the placement→store mapping is an exhaustive `match`, not a predicate chain. |
| II. Test the contract | **Binding, and previously unchecked.** Every FR now has a task and a test, including the two that had neither: FR-23 (a worker never repairs and never substitutes a same-named local repository — the live case, since `/srv/src/homelab` exists on this worker with no `worktrees/`) and FR-29 (store retention, the whole outcome of C3). Acceptance criteria 6, 9 and 10 gain tests. |
| III. Small, reversible steps | **Honoured with a caveat.** The change is larger than ideal because namespace containment is a pre-existing defect pulled in by a blast-radius change, not by preference. Recorded as a deliberate scope decision in `SPEC.md`, not an omission. Phases are ordered so each is independently shippable. |
| IV. Shared-component boundaries are law | Honoured. The probe lands in `crates/utils` precisely so `crates/worker` can call it without gaining `git`/`git2`; no crate acquires a dependency it did not have. |
| V. Remote mutations are transactional and txid-covered | Not applicable. No remote mutation; `crates/remote` is untouched. Adoption makes no database writes beyond the placement reason it already owns, which rides the caller's transaction. |
| VI. Don't rebuild what shipped | Honoured. Reuses `SharedWorkspacePaths::repository_dir`, `RepositoryAdminLockManager`, the placement state machine, `WorktreeManager`'s repair-first path, `GitCli::list_worktrees`/`fetch_with_refspec`/`list_remotes`, and — critically — `find_worktree_git_internal_name`, whose path resolution is correct and is fixed rather than replaced. The boot sweep is one sweep with one enumeration source (the database's cluster-placed workspaces), not two. |
| VII. Workspace breadcrumbs preserve issue identity | Not applicable. No breadcrumb or issue-identity surface is touched. |
| VIII. Managed tools are pinned, verified, and user-owned | Not applicable. No managed CLI is added or bumped. |
| IX. External agent protocols are defensive contracts | Not applicable. No executor or agent protocol changes. |
| X. Dialogs hold provisional state; containers hold confirmed state | Not applicable. No frontend change. |
| XI. Diagnostics are evidence, not decoration | Honoured, narrowly: placement reasons and adoption logs are surfaced as the backend produced them — not truncated, reinterpreted or auto-remediated. (The typed-absence property the probe relies on is XIX, not this principle.) |
| XII. Asynchronous handoffs have one authoritative owner | **Binding, and previously unchecked.** "Avoid holding coordination locks across awaited external operations" is exactly the `ensure` shape: the clone runs **outside** the administration lease, which is taken only for the short re-check → verify → configure → `rename(2)` → fetch. `ensure`, `adopt` and `create_workspace_fenced` each acquire and fully release the lease around their own critical section and never nest — `acquire` (`worktree_manager.rs:117-151`) takes an owned tokio mutex guard *then* an exclusive SQLite lease, so nesting deadlocks. A concurrency regression test covers both orderings, as the principle requires. |
| XIII. Vendor config files are edited, never owned | Not applicable. |
| XIV. Repository verification is worktree-safe | Honoured, and honest about its limits. `TOOLCHAIN_ENV.sh` documents the bootstrap and the `vibe-kanban-tauri` exclusion. `cargo check`/`clippy`/`test` run here; `pnpm run check`/`lint`/`format` **cannot** — this worker has no pnpm or npm — and are reported as not-run with the reason rather than silently skipped after a green summary. |
| XV. Destructive operations fail safe and are loud | Honoured. Adoption mutates no working-tree file; every outcome logs path, reason and action at `info!` before acting; retain-on-indeterminate throughout; unreachable worker means skip. Store retention (FR-29) errs the same way, so the sibling reclamation routines cannot disagree about which way to err. |
| XVI. Bundled third-party entries install what they advertise | Not applicable. |
| XVII. Live capability state is confirmed and atomic | Honoured by analogy: a store is published by `rename(2)` and is never observable half-built, and a workspace is never advertised `Ready` on an unverified worktree. |
| XVIII. Distributed execution is affinity-bound and evidence-backed | Honoured, with one correction to the record: worktree *administration* stays single-owner, but "workers do not write" was never true — a worktree's `index`/`HEAD`/`logs` live inside the store. Stated explicitly rather than quietly relied on. |
| XIX. Observability is a read-only surface | **Binding, and previously unchecked.** FR-17 makes a lifecycle decision ("skip repair, the worker is unreachable"), so the channel it reads is named: the **lease and heartbeat records** that already govern dispatch — **never** a health endpoint or a metrics surface. A node that fails to report metrics is not offline. Absence is typed throughout: `NotApplicable`, `Dangling`, `OutsideSharedRoot` and `Indeterminate` are distinct statuses carrying their reason, and a failed read never collapses into `Portable`. |
| XX. Cross-node paths are node-identical and structurally verified | This feature is the principle's first enforcement. Structural assertion, no same-named-local fallback, two-sided pointer repair, level-triggered enforcement, re-derived blast radius. |

No deviation requires an open question.

## Risks & Dependencies

The full table is in [`SPEC.md`](../../../SPEC.md). The four that shape sequencing:

1. **Auto-gc pruning another workspace's registration.** `git gc --auto` fires on
   ordinary commands and prunes worktrees; a routine `git status` on a worker
   could drop a coordinator-owned registration. The store's gc config must be
   written before the first worktree is added — this ordering is load-bearing.
2. **A misresolved registration in the consolidated `worktrees/` namespace.**
   Safe under the old layout because each repo held a handful of registrations;
   with one store per repo the namespace holds one entry per live workspace.
   VK's resolution is by path and is correct; the hazard is
   `find_worktree_git_internal_name` swallowing an NFS read failure into
   `Ok(None)`, after which the caller falls through to a broader cleanup.
   Namespace containment lands before anything touches the fleet.
3. **Rollback.** The deploy loop auto-rolls-back on a failed health probe.
   Push-back keeps a previous release working; a forward-only gate is rejected
   because it would disable that rollback (see `clarifications.md` C4).
4. **Testing near the live fleet.** `cleanup_orphan_workspaces` always sweeps the
   default base dir even with an override configured, and against a non-matching
   database would classify every live worktree as an orphan — but it is **inert**
   here: `workspace_manager.rs:714-719` early-returns when
   `allow_reclamation == false`, and `container.rs:1025` passes
   `!cluster_config.enabled`, and clustering is on across this fleet. No dev
   server runs on a cluster host during this work for a different reason: a
   second server contends for the same shared root, the same SQLite
   administration leases and the same live worktrees this change is repairing.
   All sweep behaviour is tested on fixture roots regardless.

Dependencies: none new. The shared root is already in the units' `ReadWritePaths`
and is already created by `vibe-kanban-shared-root.service`, so
`repositories/` inherits both — verified, not assumed.
