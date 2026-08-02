# Analysis — `19a4-git-worktrees-br`

Adversarial cross-check of `spec.md`, `clarifications.md`, `plan.md`,
`research.md`, `data-model.md`, `contracts/internal-seams.md` and `tasks.md`
against each other, against `.specify/memory/constitution.md` v0.18.0, and
against the actual code and the live filesystem. Every file:line claim in the
artifacts was sampled and verified.

The findings below are ordered by severity. Each carries a **Disposition**
recording what was changed in response. All ERROR-level findings are resolved in
the artifacts before implementation begins.

---

## ERROR — Factual: the affected-workspace count was wrong everywhere

`spec.md`, `SPEC.md`, `clarifications.md`, `research.md`, `plan.md` and
`tasks.md` all said "~130 broken worktrees". Measured on the live share:

```
9 cluster workspace directories, 15 worktrees, 15 broken, 0 resolving
```

The 130 figure came from PR #172's description, where it counts *all* workspaces
on `think2` — overwhelmingly `placement_state = local`, which are not affected.
The number is load-bearing twice: for the sweep's concurrency bound and for the
per-workspace-clone cost argument in `research.md`.

**Disposition.** Corrected to "every cluster-placed workspace — 15 worktrees
across 9 workspaces, 100% of them" throughout. The severity claim is unchanged
and in fact sharpened: the failure rate is total, not partial.

## ERROR — Factual: `cleanup_orphan_workspaces` is inert when clustering is enabled

`PRIOR_KNOWLEDGE.md` #28, `SPEC.md`'s risk table, `plan.md` and `tasks.md` all
called an accidental orphan sweep "the single highest-risk action available
during this task". It is not:
`workspace_manager.rs:714-719` early-returns when `allow_reclamation == false`,
and `container.rs:1025` passes `!cluster_config.enabled`. On every host in this
fleet clustering is on, so the sweep never runs.

**Disposition.** Corrected. The prohibition on starting a dev server stays — it
is still a bad idea on a coordinator — but it is no longer described as the top
risk, and the reasoning is now accurate.

## ERROR — Factual: the basename-collision defect is not where the artifacts said

`SPEC.md`, `plan.md`, `tasks.md` (T015) and `PRIOR_KNOWLEDGE.md` #16 all assert
that `create_worktree_with_retry`'s metadata force-clean derives an admin
directory name from a path basename. It does not:
`force_cleanup_worktree_metadata` (`worktree_manager.rs:755-777`) resolves
through `find_worktree_git_internal_name` (`:573-610`), which reads every
`worktrees/*/gitdir` and compares canonicalised paths. The cited range
`:718-748` is the retry block, not the cleanup function.

The *real* defects at that site are different, and worse under a consolidated
namespace: `find_worktree_git_internal_name` uses
`read_dir(...).filter_map(|entry| entry.ok())` (`:583-585`) and
`gitdir_path.exists()` (`:599`) — both idioms `PRIOR_KNOWLEDGE.md` #11 forbids
because they turn *indeterminate* into *absent*. On NFS, a transient read failure
makes the function return `Ok(None)`, and the caller then falls through to a
broader cleanup against a namespace that now holds every workspace of the repo.

**Disposition.** T015 rewritten: keep the path-resolution (it is correct, and
constitution VI says do not rebuild it), and fix the two error-swallowing idioms
plus the `git worktree prune` scope. The false premise is removed from `SPEC.md`,
`plan.md` and `PRIOR_KNOWLEDGE.md`.

## ERROR — `WorkspacePlacementState` has six variants, not five

`crates/db/src/models/workspace.rs:105-113` defines
`Local, Reserved, Provisioning, Ready, Failed, Cleaning`. Every artifact
enumerated five and omitted `Cleaning`, leaving the resolver's behaviour for a
cleaning workspace undefined. If it fell through to `repo.path`, cleanup of a
cluster workspace would run worktree administration against the wrong store —
exactly the FR-24 failure.

**Disposition.** `Cleaning` added to the worker-placement set in `spec.md`,
`plan.md`, `data-model.md` and `contracts/internal-seams.md` (R2). The resolver
is specified to be exhaustive over the enum with no wildcard arm, so a future
variant is a compile error rather than a silent fallthrough.

## ERROR — The probe cannot live in `crates/git`

`contracts/internal-seams.md` requires the probe to be callable from
`crates/worker`, and T032 forbids adding a dependency there. But T002 placed it
in `crates/git`, which pulls in `git2`. Verified: `crates/worker/Cargo.toml`
depends on `utils`, not `git`.

**Disposition.** The probe moves to `crates/utils/src/worktree_linkage.rs`,
which `crates/worker` already depends on. `crates/git` re-exports nothing; the
coordinator uses it directly from `utils`.

## ERROR — The probe's two halves have incompatible dependency requirements

`assert_toplevel_is` was specified as `git rev-parse --show-toplevel`, which
needs a subprocess, while W1 requires "filesystem reads only". Both cannot hold.

**Disposition.** Split explicitly. `probe()` is pure filesystem — reads `.git`,
the `gitdir:` line, `commondir`, and the back-pointer — and is what the worker
runs. `assert_toplevel_is` becomes a *coordinator-only* extra assertion that may
spawn `git`, and is documented as such. The ancestor-repository hazard the
subprocess guarded against is still covered on the pure path, because a `.git`
*file* naming a resolvable `worktrees/<n>` whose `gitdir` points back to this
exact worktree cannot belong to an ancestor.

## ERROR — `RepoWorktree::source_repo_path` needs an edit the plan said it did not

`plan.md` and `data-model.md` claimed `RepoWorktree` is unchanged and
`source_repo_path` "carries the right value with no further edits".
`workspace_manager.rs:444` reads `source_repo_path: input.repo.path.clone()`,
so without an edit it carries the *wrong* store into rollback and cleanup for
every cluster workspace. T010's edit list omitted `:444`.

**Disposition.** `:444` added to T010. The "unchanged" claims are corrected.

## ERROR — The `&repo.path` enumeration was incomplete

Unlisted sites found in `crates/workspace-manager/src/workspace_manager.rs`:
`:213` (`check_branch_exists` in `add_repository` — attaching a repo to an
existing cluster workspace validates the branch in the wrong store), `:255`
(`repo_paths` in `prepare_deletion_context`), `:430` (`&input.repo.path` in the
non-fenced `create_worktree` arm). In `crates/server/src/routes/workspaces/pr.rs`:
`:589` (`get_pr_comments`). In `crates/local-deployment/src/container.rs`:
`:2060` (`copy_project_files`, which must *stay* on `repo.path` — a bare store
has no working tree — but the decision was unrecorded).

**Disposition.** All five added to the appropriate list in `SPEC.md`, `plan.md`
and `tasks.md`, `:2060` to the deliberately-unconverted list.

## ERROR — Ordering hazards in the task graph

Four, all real:

1. **T007 (Git plumbing) is listed after T006, which uses it.** T006 must depend
   on T007.
2. **T029 (branch push-back) is sequenced after T027/T028, which adopt the live
   fleet.** Push-back exists precisely to make a rollback safe; adopting before
   it lands opens the window FR-28 is meant to close. T027 and T028 must depend
   on T029.
3. **Phase 4 (namespace containment) claims it must precede anything touching
   the live fleet, but T012 in Phase 3 is what consolidates the namespace.**
   Phase 4 must precede Phase 3.
4. **Phases 3 and 5 are not independently shippable.** Between them, new cluster
   branches exist only in the store while every route still reads `repo.path`,
   regressing branch status, diff and PR. Constitution III wants shippable steps.

**Disposition.** Phases reordered to 1 → 4 → 2 → 3+5 (merged, since they must
land together) → 6 → 7. Dependencies corrected on T005, T006, T024, T027, T028.

## ERROR — `SharedRepositoryStore` contracts are unimplementable as signed

- `new(shared_root)` and `ensure(&self, repo, target_branch)` carry no lock
  manager and no pool, yet E2 requires the administration lease.
- `adopt(&self, worktree_path, branch)` cannot know which repository's store to
  adopt *into*: the worktree's existing pointer names `/srv/src/<repo>`, not the
  store, so the store is underivable from the arguments. It also needs the repo
  to read the old tip from and to push back to.

**Disposition.** Signatures corrected to
`SharedRepositoryStore::new(shared_root, locks)` and
`adopt(&self, repo: &Repo, worktree_path: &Path, branch: &str)`.

## ERROR — Lock nesting is undefined and would deadlock

`SPEC.md` says `adopt` runs "under the repository admin lock after `ensure`",
while `ensure` step 1 acquires that same lock. `RepositoryAdminLockManager::acquire`
(`worktree_manager.rs:117-151`) takes an owned `tokio::sync::Mutex` guard and
then an exclusive SQLite lease, so a nested acquire for one repository deadlocks
on the in-process mutex and a concurrent one returns `RepositoryLockBusy`.
`create_workspace_fenced` wants the same lock again.

**Disposition.** A non-nesting discipline is now stated explicitly: `ensure`,
`adopt` and `create_workspace_fenced` each acquire and fully release the lease
around their own critical section, and no one of them is called while another
holds it. Recorded in `contracts/internal-seams.md` as a precondition, with a
regression test for concurrent provisioning of one repository.

## ERROR — The cross-crate dedup mechanism C1 relies on does not exist

`clarifications.md` C1 says concurrent provisionings are "deduplicated by the
existing in-process per-repository operation mutex". `repository_operation_lock`
and its `REPOSITORY_OPERATION_LOCKS` static are private to
`crates/worktree-manager` (`worktree_manager.rs:189`), so
`crates/workspace-manager` cannot reach them.

**Disposition.** C1 corrected: dedup comes from the administration lease itself
(the loser observes the winner's store on its early-out re-check), not from a
mutex it cannot see. No new mutex is minted.

## ERROR — A `Failed` workspace can never be lazily adopted

`ensure_container_exists` (`container.rs:2632-2637`) returns `Err` for any
cluster placement state other than `Ready`. T013 makes a non-portable workspace
`Failed`; T027 wires lazy adoption into `ensure_container_exists`. A workspace
that fails the new assertion is therefore permanently unrecoverable, and no task
defines a `Failed → Ready` transition after a successful repair.

**Disposition.** New task: allow a `Failed` cluster workspace to re-enter
provisioning when adoption succeeds, and add an acceptance criterion that a
workspace failed by the portability assertion is recoverable without operator
surgery.

## ERROR — FR-23 and FR-29 have no tasks

- **FR-23** ("a worker MUST NOT repair; MUST NOT substitute a same-named local
  repository") — the *exact* live case, since `/srv/src/homelab` exists on this
  worker with no `worktrees/`.
- **FR-29** (store retention) — the whole outcome of clarification C3, promised
  by `plan.md`, implemented by nothing. Also a constitution XV violation: the
  sibling reclamation routines are guaranteed to disagree about which way to err.

**Disposition.** Tasks added for both, with tests.

## ERROR — `vk-deploy-notify` is not a Rust API

T033 and `IMPLEMENTATION_PLAN.md` say to route an alarm through "the existing
null-topic-safe deploy notifier … in `crates/local-deployment/src/container.rs`".
`vk-deploy-notify` appears nowhere in `crates/`; it is a systemd/Nix helper in
`homelab/modules/vibe-kanban-rebuild.nix`, and `SPEC.md` puts that module out of
scope.

**Disposition.** T033 removed. The sweep reports through structured logging and
the placement reason, which are in-process and already surfaced. Notification is
recorded as a possible follow-up, not a task in this change.

## ERROR — Phase 8 drops two of the three mandated verification gates

`SPEC.md` AC 8 requires `cargo test --workspace`, `pnpm run check` and
`pnpm run lint`. Phase 8 had no `check` and no `lint`, and its
`pnpm run format` task is unexecutable: this worker has no pnpm or npm, as
`tasks.md` itself states two lines earlier. Constitution XIV forbids exactly
this — "verification must never silently skip a language or package after
reporting overall success".

**Disposition.** Phase 8 now states plainly which gates run here (`cargo check`,
`clippy`, `test`) and which **cannot** run in this environment (`pnpm run
check`/`lint`/`format`), with the reason. The change is backend-only, so no
generated types move; that is asserted by inspection rather than by a tool. The
gap is reported in the task's completion notes rather than being papered over.

## ERROR — CI path filters miss most of the changed crates

T004 said to add `crates/git/**` to the `backend` filter. It is already there
(`.github/workflows/test.yml:64`), so T004 was a no-op. Actually missing:
`crates/workspace-manager/**`, `crates/worktree-manager/**`, `crates/worker/**`
and `crates/cluster-protocol/**` — which hold the store, the namespace
containment and the worker preflight, i.e. most of this change and most of its
tests. `PRIOR_KNOWLEDGE.md` #60 warns about precisely this.

**Disposition.** T004 rewritten to add the four genuinely missing crates. This is
a pre-existing hole that this change is the first to be bitten by.

## ERROR — `discover_repo_names` defeats the preflight one frame above it

`crates/worker/src/execution.rs:479-484` is
`let Ok(mut entries) = tokio::fs::read_dir(workspace).await else { return names; }`
and `path.join(".git").exists()`. A read failure on NFS yields an **empty** list,
so the preflight probes nothing and the dispatch is accepted — the "indeterminate
never collapses into portable" guarantee is defeated before the probe runs.

**Disposition.** New task: make `discover_repo_names` distinguish "no repos" from
"could not enumerate", and have preflight refuse on the latter.

## WARNING — `[P]` markers are mostly false by the document's own definition

`[P]` means "touch independent files", but T003/T002, T008/T006, T017/T016,
T030/T025-T029, T034/T032 and T014/T021/T024/T033 all share a file with a task
they run beside. T033 was `[P]`, depended on T031, and edited T031's file.

**Disposition.** `[P]` re-applied only where the files really are disjoint.

## WARNING — plan.md checks 7 of 20 constitution principles

Unchecked and binding: **II** (test the contract — FR-23, FR-29 and acceptance
criteria 6, 9, 10 have no validation), **XII** (locks across awaits — see below),
**XIX** (observability is not evidence — FR-17 makes a lifecycle decision from
worker reachability without naming the channel it reads). The **XI** row is also
decorative: XI is about preserving exact backend text in the UI; the typed-absence
property `plan.md` invokes is XIX.

**Disposition.** All twenty principles are now checked, XI is restated correctly,
and FR-17 is pinned to the lease/heartbeat evidence channel — never a health or
metrics probe.

## WARNING — Constitution XII: the lease is held across an awaited clone

`SPEC.md` §2 and `IMPLEMENTATION_PLAN.md` step 7 acquire the lease first and
clone inside it, contradicting `spec.md` FR-6, `clarifications.md` C1,
`research.md` D3 and contract E2 — all of which say the opposite. The two
repo-root documents were simply never updated after C1 was resolved.

**Disposition.** `SPEC.md` and `IMPLEMENTATION_PLAN.md` corrected to
clone-outside / publish-inside. A concurrency regression test is added, as XII
requires.

## WARNING — Constitution VI: three helpers would be rebuilt

Verified to exist already: `GitCli::list_worktrees` (`cli.rs:318`),
`GitCli::fetch_with_refspec` (`cli.rs:365`), `GitCli::list_remotes` (`cli.rs:455`),
`GitService::list_remotes` (`lib.rs:1463`), and
`WorktreeManager::find_worktree_git_internal_name` (`worktree_manager.rs:573`).
T007 and T015 proposed reimplementing several of them. T028 and T031 also added
*two* fleet-wide boot sweeps in one file with different enumeration sources
(filesystem vs database) — against `PRIOR_KNOWLEDGE.md` #33.

**Disposition.** T007 reduced to the genuinely new plumbing (bare clone, config
set, `cat-file -e`). T028 and T031 merged into one sweep with one enumeration
source: the database's cluster-placed workspaces, which is the authority.

## WARNING — Coverage gaps in acceptance

No test task for: clustering *disabled* (AC 9 — distinct from `local` placement
with clustering enabled); rebase, merge, push or PR creation (AC 10 — T020
converts six `pr.rs` sites with no test at all); `adopt` guarantees A6, A8, A9.

**Disposition.** Tasks added; A6/A8/A9 assigned explicitly to T025.

## WARNING — The two-node gate was weakened

`SPEC.md` and `PRIOR_KNOWLEDGE.md` #58 require the gate to disconnect the
coordinator and cancel a process group; T040 dropped both.

**Disposition.** Restored in full.

## WARNING — Constitution XX: "umask 002" on a directory is not a mechanism

E5 and `data-model.md` specify the store as "setgid, group-owned, umask 002".
A directory has no umask. Setgid fixes group *ownership* of files workers create;
it does not fix their *mode*, and
`homelab/modules/vibe-kanban-rebuild.nix:706-722` sets no `UMask=` on
`vibe-kanban-worker`. `SPEC.md` puts the homelab module out of scope, so nothing
in this change can set it.

**Disposition.** Restated honestly. Setgid is applied for group ownership. The
process umask is set **in-process** by the coordinator around store creation, and
the worker's mode-on-create for lock files is recorded as a **residual risk** with
the concrete follow-up (a `UMask=002` on the worker unit) named. The measured
four-way concurrent-commit run is evidence it does not bite today — the export
maps every writer to the same storage-side identity — but it is not proof.

## WARNING — Minor citation errors

`plan.md:56` cites `worktree_manager.rs:107-151` for `acquire`; it is `:117-151`
(`:107` is `pub fn new`). `SPEC.md` cites `think2.nix:235-247` for the 15-minute
reset cadence; those lines contain only `forceSync = true; ref = "main";` — the
cadence is in the `git-projects` timer. `IMPLEMENTATION_PLAN.md` and
`PRIOR_KNOWLEDGE.md` name `PtyService` without its file
(`crates/local-deployment/src/pty.rs`), and T023 sent the auditor to two other
files.

**Disposition.** All corrected.

## INFO — Unrequirement'd but retained

`SPEC.md` §7 (provisioning logs the store path, remotes fetched, resolved OID)
has no FR. Retained as implementation guidance rather than promoted, since
constitution XIX makes logging explicitly non-evidential.

`PRIOR_KNOWLEDGE.md` says `docs/knowledge-base/` has 21 pages; it has 20 content
pages plus `INDEX.md`. Corrected.

---

## Verdict

The design is sound and the root cause is confirmed by reproduction. The
artifacts, however, carried six factual errors about the code and the live
system, four ordering hazards, two unimplementable contract signatures, two
untasked requirements, and three constitution violations. All ERROR-level
findings are resolved before implementation. The single most valuable correction
is the affected-workspace count: the failure is not "~130 of many", it is **every
cluster-placed workspace, without exception**.
