# Technical Spec: Repair Git worktrees in cluster mode

Task id: `19a4-git-worktrees-br`

> Constraints distilled from the project knowledge base are in
> [`PRIOR_KNOWLEDGE.md`](PRIOR_KNOWLEDGE.md); the load-bearing ones are folded
> into the design sections below and cited where they apply.

## Summary

Since [#169](https://github.com/davidvasandani/vibe-kanban/pull/169) (clustered
Vibe Kanban) and [#172](https://github.com/davidvasandani/vibe-kanban/pull/172),
every workspace that Vibe Kanban places on a worker node contains a Git worktree
that is unusable on that worker. Any `git` command an agent runs inside its
workspace fails with:

```
fatal: not a git repository: (null)
```

The cause is a path-portability defect, not a protocol or scheduling defect. The
coordinator creates the worktree with `git -C <repos.path> worktree add
<shared_path>`, where `repos.path` is a **coordinator-local** path such as
`/srv/src/vibe-kanban`. Git records absolute paths in both directions:

- `<workspace>/<repo>/.git` contains `gitdir: /srv/src/vibe-kanban/.git/worktrees/<name>`
- `/srv/src/vibe-kanban/.git/worktrees/<name>/gitdir` contains the shared path

Only `/srv/vibe-kanban-shared` is mounted on worker nodes. `/srv/src` is local,
per-host storage, so the `gitdir:` pointer either does not exist on the worker or
— worse — resolves to a *different* repository that happens to share the name.
The worktree's object store, refs, index and HEAD are therefore unreachable and
the workspace has no Git at all.

This spec makes Vibe Kanban own a canonical, node-identical Git store per
repository under the shared root, creates cluster worktrees from that store,
routes workspace-scoped Git operations at it, enforces the portability invariant
so the failure can never be silent again, and heals the already-broken
workspaces in place without destroying agent work.

## Motivation

- **Every** cluster-placed workspace is affected, without exception. Measured on
  the live share: 15 worktrees across 9 cluster workspace directories, 15 broken,
  0 resolving. Agents cannot `git status`, `git diff`, `git commit`, or read
  history; the diff view, branch status, rebase, merge and PR flows all fail for
  these workspaces. (The "~130 workspaces" figure in
  [#172](https://github.com/davidvasandani/vibe-kanban/pull/172)'s description
  counts *all* workspaces on `think2`, overwhelmingly `placement_state = local`,
  which are unaffected. The failure rate among cluster placements is 100%, not
  partial.)
- The failure mode is silent and late. Provisioning reports `Ready`, the
  scheduler reports a healthy mount, and the agent only discovers the problem
  when it runs its first Git command — several minutes and several tool calls
  into a turn.
- The current behaviour depends on an **unenforced operator convention**.
  `docs/self-hosting/clustered-workers.mdx:34` tells the operator to create a
  `repositories` directory inside the NFS mount, but nothing registers repos
  there, nothing validates that a registered repo is reachable from a worker,
  and `SharedWorkspacePaths::repository_dir()` is dead code
  (`crates/workspace-manager/src/workspace_manager.rs:53-55`, referenced only by
  `create_base_dirs` and unit tests).
- Even if the operator followed the convention, the homelab deployment cannot:
  `/srv/src/vibe-kanban` is declared `forceSync = true; ref = "main"` in
  `homelab/hosts/think/think2.nix:235-247`, and the `git-projects` timer
  `git reset --hard origin/main`s every such checkout roughly every 15 minutes.
  `modules/vibe-kanban-rebuild.nix:10-13` declares that checkout "BUILD INPUT
  ONLY". It is the wrong place for workspace branches to live.

## Scope

- A coordinator-owned **shared repository store** at
  `{shared_root}/repositories/{repo_id}`, provisioned on demand, fenced by the
  existing `RepositoryAdminLockManager`.
- Cluster worktree creation, `ensure`, and cleanup performed against that store.
- Workspace-scoped Git operations (branch status, diff, rebase, merge, push, PR)
  routed to the store for cluster-placed workspaces.
- A **portability precondition**, enforced level-triggered (boot, placement,
  dispatch), that fails loudly when a workspace's worktrees cannot be made
  node-identical.
- **In-place adoption** of the already-broken worktrees, preserving tracked,
  committed and untracked working-tree content.
- Worker-side preflight so an unresolvable worktree is reported as a typed
  dispatch failure rather than discovered by the agent.
- **Scoping the repo-wide `git worktree prune`** in
  `comprehensive_worktree_cleanup`. This is pre-existing and known-unfixed
  (`docs/knowledge-base/workspace-directory-reclamation.md`), but consolidating
  every cluster workspace of a repo onto one store raises its blast radius from
  per-node to cluster-wide, so it becomes this change's problem.
- Tests, docs, and knowledge-base updates.

## Out of scope

- Any change to the dispatch protocol, signing, heartbeat, lease, scheduler
  weighting, preview routing, or `worker_nodes` schema.
- Multi-coordinator/HA Git administration. The single-writer coordinator
  contract from `957e-clustered-vibe-k` is retained unchanged.
- Changing `homelab/modules/vibe-kanban-rebuild.nix`. The fix is entirely
  inside Vibe Kanban; the shared root, mount, and NFS export stay as they are.
  `/srv/src` deliberately remains local, build-input-only storage.
- Migrating **local** (`placement_state = local`) workspaces off
  `/var/tmp/vibe-kanban/worktrees`. They are correct today and stay as they are.
- Garbage collection / repacking policy for the shared store beyond disabling
  automatic gc.

## Background

### How a cluster workspace is provisioned today

```
create_cluster_workspace                         container.rs:3626
  └─ SharedWorkspacePaths::workspace_dir(id)     → {shared_root}/workspaces/{uuid}
  └─ workspace_repo_inputs(workspace.id)         container.rs:635-669
       → RepoWorkspaceInput { repo, target_branch }   (repo.path is verbatim DB text)
  └─ WorkspaceManager::create_workspace_fenced   workspace_manager.rs:376
       └─ WorktreeManager::create_worktree_fenced(lock, repo.id, &repo.path, …)
            └─ RepositoryAdminLockManager::acquire(repo_id, repo_path)
            └─ GitService::create_branch(repo.path, branch, target)
            └─ GitService::add_worktree(repo.path, worktree_path, branch, false)
                 └─ git -C <repo.path> worktree add <worktree_path> <branch>
```

`workspace.container_ref` is set to the workspace directory and is the only path
transmitted to the worker (`crates/cluster-protocol/src/lib.rs:109-110`). The
worker authorises it against `shared_root`
(`crates/worker/src/path_authority.rs:63-89`) and `cd`s there. The worker runs
**no** Git administration commands — by design, per `957e-clustered-vibe-k`.

Nothing between the coordinator's `repos.path` and the worker's `cd` rewrites,
validates, or even inspects the worktree's Git linkage.

### Reproduction (observed on this worker, `think-cluster`)

```
$ cat /srv/vibe-kanban-shared/cluster/workspaces/19a4a176-…/vibe-kanban/.git
gitdir: /srv/src/vibe-kanban/.git/worktrees/vibe-kanban8
$ ls /srv/src/vibe-kanban
ls: cannot access '/srv/src/vibe-kanban': No such file or directory
$ git -C /srv/vibe-kanban-shared/cluster/workspaces/19a4a176-…/vibe-kanban status
fatal: not a git repository: (null)
```

The `homelab` worktree in the same workspace shows the more dangerous variant:
`/srv/src/homelab` *does* exist on this worker (a different clone, managed by
`services.gitProjects`) but has no `.git/worktrees/` directory, so the pointer is
still dangling — and would have silently bound the workspace to an unrelated
repository had the registration existed.

### Why the obvious alternatives are rejected

| Alternative | Rejected because |
| --- | --- |
| Export `/srv/src` over NFS as well | The NFS server is a NAS appliance (`172.16.0.99`), not the coordinator; the coordinator would have to run `nfsd`. It also breaks the single-writable-shared-root model that mount health is built on, and collides with each worker's own `/srv/src`. |
| Rewrite the `gitdir:` pointer per node at dispatch time | The object store still lives on coordinator-local disk. There is nothing to point at. |
| Give each workspace a full independent clone under `workspaces/{id}/{repo}` | Correct but expensive: no hardlinks across the NFS boundary, so every workspace pays a full object-store copy — 15 today, and the cost is per new workspace forever, not a one-off. It also breaks `check_branch_exists(&repo.path, …)`, `get_base_commit(&repo.path, …)` and every PR/merge call site, because the workspace branch would not exist in any store the coordinator queries. |
| Let workers create their own worktrees | Explicitly rejected in `specs/vk/957e-clustered-vibe-k/research.md:72` — races shared `.git/worktrees` metadata. |
| Documentation-only fix ("register repos under the shared mount") | Leaves an unenforced convention whose violation is silent and destroys a turn's work. The deployment cannot follow it anyway (see Motivation). |

## Design

### 1. The portability invariant

> **P1.** A workspace that may execute on a worker must have every worktree
> backed by a Git common directory whose absolute path resolves identically on
> the coordinator and on every worker — i.e. it must lie inside `shared_root`.

P1 is the single statement this change enforces. Everything below either
establishes it, checks it, or repairs a violation of it.

P1 is about the **common directory**, not the repo path, because
`GitService::get_common_dir` is already the identity Git administration is keyed
on (`crates/worktree-manager/src/worktree_manager.rs:178-189`). A repo registered
as one of its own worktrees resolves to the same store.

### 2. Shared repository store

New module `crates/workspace-manager/src/shared_repository.rs`, type
`SharedRepositoryStore`.

**Layout.** `{shared_root}/repositories/{repo_id}` — the path
`SharedWorkspacePaths::repository_dir` already computes. The directory is a
**bare** repository (`HEAD`, `objects/`, `refs/`, `worktrees/`). Bare is correct:
the store is never checked out, `worktrees/` sits at the top level, and there is
no working tree for anything to reset.

**Provisioning** — `SharedRepositoryStore::ensure(repo, target_branch)`. The
long-running clone runs **outside** the administration lease; the lease covers
only the short verify-and-publish step. Constitution XII forbids holding a
coordination lock across an awaited external operation, and the lease is a
bounded SQLite lease that a multi-minute clone of a large repository could
silently outlive — unfencing exactly the operation the lease exists to fence.

1. Early-out, unlocked: if the store already exists and resolves `target_branch`
   (`git cat-file -e <sha>^{commit}`), return it. This runs on every provisioning
   and must be cheap.
2. **Outside the lease**, clone into the per-attempt staging directory
   `repositories/.{repo_id}.incoming`. Staging names are per-repository, never a
   shared `tmp/`, so concurrent attempts cannot corrupt each other.
   - Seed from `repo.path` (`git clone --bare <repo.path> <tmp>`) so the clone is
     a local-disk read rather than a network fetch, and so repos with no remote
     work.
   - Create the staging directory setgid, group-owned, with umask `002`,
     **before** the first object is written. Every node writes into this store
     (see "Concurrency"), and shared writable state in this fleet has failed on
     lock *inode* modes before, not on directory permissions
     (`homelab/docs/knowledge-base/vk-app-managed-cli-tools.md`). Retrofitting
     after the first `packed-refs.lock` exists does not fix it.
3. **Acquire `RepositoryAdminLockManager::acquire(repo.id, …)`** and, under it,
   do only work that is short and bounded:
   - Re-check the early-out. A concurrent `ensure` for the same repository may
     have published while this one was cloning; the loser observes the winner's
     store here and discards its own staging directory. This — not any in-process
     mutex — is what deduplicates concurrent provisioning.
   - Verify the staged clone: real refs, a resolvable HEAD, and the target branch
     proven with `cat-file -e`. A created directory is not evidence.
   - Copy every remote name/URL from `repo.path` into the store, replacing the
     `origin` that `git clone --bare` points at the local checkout. Push, PR
     creation and `check_remote_branch_exists` must reach the real forge.
   - Set `gc.auto=0`, `gc.autoDetach=false`, `maintenance.auto=false`, and
     `gc.worktreePruneExpire=never`. `git gc --auto` fires opportunistically on
     ordinary commands and *does* prune worktrees, so without this a routine
     `git status` **on a worker** could drop another workspace's registration in
     the coordinator-owned store. This is the single most important config on the
     store.
   - Set `core.logAllRefUpdates=true` so reflogs exist for recovery.
   - `rename(2)` the staging directory into place. A half-written store on NFS
     must never be observable as a valid one.
   - Fetch what the workspace needs: `+refs/heads/*:refs/heads/*` from `repo.path`
     while that path is still a valid repository, so branches created by
     coordinator-local workspaces stay visible; and the `target_branch` from the
     real remote when one exists. Per-remote failure is tolerated, but the
     **target branch must resolve** in the store afterwards or `ensure` fails.
4. Release the lease, returning the store path.

`ensure` is idempotent and level-triggered: safe to call on every provisioning
and every `ensure_container_exists`.

**Lock discipline.** `ensure`, `adopt` and `create_workspace_fenced` each acquire
the administration lease around their own critical section and **fully release it
before returning**. None is ever called while another holds it.
`RepositoryAdminLockManager::acquire`
(`crates/worktree-manager/src/worktree_manager.rs:117-151`) takes an owned
`tokio::sync::Mutex` guard and *then* an exclusive SQLite lease, so a nested
acquire for one repository deadlocks on the in-process mutex, and a concurrent
one returns `RepositoryLockBusy`. The call order at provisioning is therefore
`ensure` → (lease released) → `create_workspace_fenced`, never one inside the
other.

**Concurrency.** The split is *single-writer administration, many ordinary
writers* — not "coordinator writes, workers read". A linked worktree keeps its
`index`, `HEAD`, `ORIG_HEAD` and `logs/` inside `{store}/worktrees/<n>/`, so
every worker Git command — including `git status` — writes into the shared store.
What stays single-writer is administration: only the coordinator runs
`worktree add`/`remove`/`prune` or deletes shared branches, fenced by
`RepositoryAdminLockManager`. Ordinary contention is per-branch and per-worktree
because each workspace owns a distinct branch, and `packed-refs` is rewritten
only by the coordinator because automatic gc is disabled.

Measured on the production export (`172.16.0.99:/var/nfs/shared/VibeKanban`,
NFSv3, `hard`): four worktrees of one bare store committing 15 times each
concurrently produced 60 commits, five correct branch heads, and a clean
`git fsck`. This is evidence the shape works on this storage, not proof — the
two-node gate below is still required.

**Shared `worktrees/` namespace.** Git derives an admin directory name from the
worktree path's *basename*, and every cluster worktree of a repo is named
`<repo_name>`, so one store accumulates `<repo>`, `<repo>1`, `<repo>2`… Under the
old layout each repo checkout held a handful of registrations and cleanup
resolved them by path; with one store per repo this namespace holds one entry per
live workspace. Two consequences, both in scope:

- `force_cleanup_worktree_metadata` (`worktree_manager.rs:755-777`) is **already**
  path-resolved: it goes through `find_worktree_git_internal_name` (`:573-610`),
  which reads every `worktrees/*/gitdir` and compares canonicalised paths. That
  resolution is correct and must not be rebuilt. The defect at that site is
  different, and worse under a consolidated namespace: the resolver uses
  `read_dir(...).filter_map(|entry| entry.ok())` (`:583-585`) and
  `gitdir_path.exists()` (`:599`), both of which turn *indeterminate* into
  *absent*. On NFS a transient read failure makes the function return `Ok(None)`,
  and the caller then falls through to a broader cleanup against a namespace that
  now holds every workspace of the repo. Both idioms must become `try_exists()`
  and a propagating `read_dir`, so an unreadable namespace is an error rather
  than an empty one. `comprehensive_worktree_cleanup` must be audited on the same
  terms.
- `comprehensive_worktree_cleanup`'s trailing repo-wide `git worktree prune`
  becomes cluster-wide. It must be scoped to the worktree being cleaned. The same
  prune already killed a production build in this fleet on 2026-07-05 when it
  walked foreign-owned registrations (`wiki/self-hosted-deployment.md`).

### 3. Resolving the Git path for a repo

Add to `RepoWorkspaceInput`:

```rust
pub struct RepoWorkspaceInput {
    pub repo: Repo,
    pub target_branch: String,
    /// Git directory that owns this repo's worktree administration for this
    /// workspace. Equals `repo.path` for coordinator-local workspaces and the
    /// shared store for cluster-placed ones.
    pub git_path: PathBuf,
}
```

`RepoWorkspaceInput::new(repo, target_branch)` keeps today's behaviour
(`git_path = repo.path`); `RepoWorkspaceInput::shared(repo, target_branch, store)`
is used by the cluster path. Inside `workspace_manager.rs`, every `&repo.path`
that feeds worktree administration becomes `&input.git_path`: `:420`, `:430`
(the non-fenced `create_worktree` arm, easily missed because it reads
`&input.repo.path` rather than a bare `&repo.path`), `:540`, `:546`, `:554`,
`:571`, `:581`, `:670`, `:676`.

Three further sites in the same file do **not** feed `create_worktree` and were
missed by earlier drafts:

- `:213` — `check_branch_exists(&repo.path, …)` in `add_repository`. Attaching a
  repo to an existing cluster workspace otherwise validates the branch in the
  wrong store.
- `:255` — `repo_paths` in `prepare_deletion_context`.
- `:444` — `source_repo_path: input.repo.path.clone()` in the `RepoWorktree`
  constructor. This one is **required**, contrary to the earlier claim that
  `RepoWorktree` needs no edit: it reads `input.repo.path`, not
  `input.git_path`, so without the change it carries the coordinator-local store
  into rollback and cleanup for every cluster workspace. With it changed,
  `source_repo_path` propagates correctly to `:695`.

`WorkspaceManager::cleanup_workspace(workspace_dir, repos: &[Repo])` cannot
derive the store from a bare `Repo`, so its signature changes to accept the
resolved inputs. Caller: `crates/local-deployment/src/container.rs:911`.

**Server-side resolution.** One helper, used by every route that touches a
workspace's branch:

```rust
async fn workspace_repo_git_path(&self, workspace: &Workspace, repo: &Repo)
    -> Result<PathBuf, ContainerError>
```

It returns the shared store when clustering is enabled *and* the workspace's
placement is a worker placement, and `repo.path` otherwise. `WorkspacePlacementState`
has **six** variants (`crates/db/src/models/workspace.rs:105-113`): `Local`,
`Reserved`, `Provisioning`, `Ready`, `Failed` and `Cleaning`. The worker-placement
set is `Reserved`/`Provisioning`/`Ready`/`Failed`/`Cleaning` — `Cleaning` included,
because cleanup of a cluster workspace must run worktree administration against
the store it was created from; falling through to `repo.path` there is exactly the
FR-24 failure. `Local` keeps today's behaviour verbatim — this is the exact
distinction #172 established and it must not be re-broken. The resolver matches
the enum **exhaustively, with no wildcard arm**, so a future variant is a compile
error rather than a silent fallthrough.

Call sites to convert (all currently `&repo.path`):

- `crates/server/src/routes/workspaces/git.rs:207,227,441,445,452,470,520,536,605,720,750`
- `crates/server/src/routes/workspaces/pr.rs:204,418,439,574,589,706,709` —
  `:589` is `get_pr_comments`, which resolves the PR from the repository the
  workspace's branch lives in
- `crates/local-deployment/src/container.rs:3367` (`get_base_commit`) and `:3386`
  (`DiffStreamArgs::repo_path`, which resolves the diff base alongside the
  already-correct `worktree_path`)

Deliberately **not** converted, because they describe the operator's checkout
rather than the workspace branch: `crates/server/src/routes/repo.rs:167` (open in
editor) and `:223` (`search_repo`) — a bare store has no working tree. Repo-level
branch and remote listing (`repo.rs:92,105,260,263`) also stays on `repo.path`;
the store's remotes are copied from it, so the answers agree. So does
`crates/local-deployment/src/container.rs:2060` (`copy_project_files`), which
copies files *out of the registered checkout's working tree* into the new
worktree — a bare store has no working tree to copy from, so routing it at the
store would break `copy_files` for every cluster workspace.

### 4. Enforcement

`create_cluster_workspace` (`container.rs:3626`) calls
`SharedRepositoryStore::ensure` for each repo before `create_workspace_fenced`,
and passes the store paths through `RepoWorkspaceInput::shared`.

After the worktrees are created, and before the placement transitions to
`Ready`, assert P1 for every worktree. The probe is structural, not a string
match — asserting "the resolved common dir is under `{shared_root}`", never
"the path does not contain `/srv/src`":

```rust
WorktreeLinkage::probe(&worktree_path)?   // .git → gitdir: → commondir → back-pointer
  .assert_common_dir_within(shared_root)? // → WorkspaceError::WorktreeNotPortable
  .assert_toplevel_is(&worktree_path)?    // coordinator only; may spawn git
```

The probe lives in **`crates/utils/src/worktree_linkage.rs`**, not in
`crates/git`. `crates/worker` depends on `utils` and must not gain `git` (and
through it `git2`); `crates/git` needs nothing re-exported, it calls `utils`
directly.

The two halves have different dependency requirements and are therefore split
explicitly:

- **`probe()` is pure filesystem** — it reads `.git`, parses the `gitdir:` line,
  resolves `commondir`, and follows the store's `worktrees/<n>/gitdir` back. No
  subprocess. This is the half the worker runs.
- **`assert_toplevel_is` is a coordinator-only extra assertion** that may spawn
  `git rev-parse --show-toplevel`. It is never on the worker path.

The ancestor-repository hazard `assert_toplevel_is` guards against is still
covered on the pure path: `git -C` walks *up* to find a repository, but a `.git`
**file** naming a resolvable `worktrees/<n>` whose `gitdir` points back to *this
exact worktree* cannot belong to an ancestor — an ancestor's registration would
name the ancestor's path. The two-sided pointer check is what closes the hazard;
the subprocess is defence in depth where a subprocess is affordable.

Three details the knowledge base makes non-negotiable:

- **Assert both directions.** `{worktree}/.git` → `{store}/worktrees/<n>` *and*
  `{store}/worktrees/<n>/gitdir` → `{worktree}/.git`. Repairing only one leaves a
  dangling registration, which is the precondition of the prune incident.
- **Prove the objects.** `git rev-parse` echoes any well-formed 40-hex string
  whether or not the object exists; presence is proven with
  `git cat-file -e <sha>^{commit}`. Creating `repositories/{repo_id}` is not
  evidence — assert the store resolves the workspace's branch tip.
- **`try_exists()`, never `exists()`**, and never
  `read_dir(..).filter_map(|e| e.ok())`. NFS stat failures are routine here and
  both traps convert "indeterminate" into "clean".

A violation fails provisioning with `placement_state = failed` and a
`placement_reason` naming the repo and the offending common dir. A workspace that
cannot be made portable must never be advertised as `Ready`: an unverifiable
state is reported as failed, never as success.

**Level-triggered, not creation-time.** Checking only at creation is an edge
trigger, and edge triggers stall silently. The same probe runs on coordinator
boot over all cluster-placed workspaces, at placement, and at dispatch, with
per-workspace failure isolation. It enumerates *every* non-portable worktree in
one pass and emits a concrete repair action rather than aborting on the first.
A one-off migration with no recurring check regresses the moment an unfixed path
creates a worktree.

`RepoService::validate_git_repo_path` is left alone — repos may still be
registered anywhere. Portability is a property of the *placement*, not of the
registration. A local-placed workspace probes as `NotApplicable`, a distinct
status from `Broken`; every probe is bounded by a timeout because NFS calls hang.

### 5. Adoption of already-broken worktrees

The 15 existing worktrees — every cluster-placed one, across 9 workspaces —
contain real content that recreation would destroy: agent edits, `node_modules`,
build caches, and, where the agent committed before the breakage, commits
reachable only from the coordinator's `/srv/src/<repo>`. Adoption re-links them
in place.

`SharedRepositoryStore::adopt(repo, worktree_path, branch)`. It takes `&Repo`
because the store is *not* derivable from the worktree alone: a broken worktree's
pointer names `/srv/src/<repo>`, which identifies no store, and adoption also
needs the repo to read the old tip from and to push back to. It runs **after**
`ensure` has returned and released the lease, and acquires the lease itself for
its own critical section — never nested inside `ensure`'s (see "Lock discipline"
above):

1. Probe the worktree. Adopt only when `.git` is a **file** whose `gitdir:`
   target is missing or resolves outside `shared_root`. A healthy worktree, a
   real `.git` directory, and a non-repository directory are all left untouched.
   Already-portable worktrees early-out, so the sweep stays cheap as the fleet
   grows.
2. Guarantee the branch tip exists in the store. Read the tip from the old common
   dir when it is still readable on this node (the coordinator can read
   `/srv/src/<repo>`), fetch it into the store, and prove it landed with
   `git cat-file -e <sha>^{commit}`. If the branch cannot be resolved, adoption
   stops and reports — it never falls back to the target branch, because that
   would silently discard commits. Refuse if the branch is already checked out by
   another worktree of the store: branch-checkout exclusivity is now fleet-wide,
   and adoption must not steal a branch from a live workspace.
3. Write the linkage:
   - `{store}/worktrees/{name}/` containing `commondir` (`../..`), `gitdir` (the
     absolute path of `{worktree}/.git`), and `HEAD` (`ref: refs/heads/{branch}`);
   - `{worktree}/.git` = `gitdir: {store}/worktrees/{name}`.
   `{name}` is derived from the workspace UUID and repo name, so it is unique and
   identical on every node — unlike the basename Git would pick.
   Every write is same-directory temp file + `rename(2)`. The `.git` marker is
   **never** transiently unlinked: a directory with no `.git` in any subdirectory
   is classified as holding no work, and a concurrent orphan sweep would delete
   it.
4. `git -C {store} worktree repair {worktree}` so Git normalises both directions.
5. `git -C {worktree} reset` (mixed; no working-tree change) to rebuild the
   absent index. Without this, the worktree reports every tracked file as both
   deleted and untracked.
6. Re-probe and assert P1 — a zero exit from `worktree repair` is not
   verification.
7. Push the branch back into `repo.path` (a local, cheap ref update). This is
   purely for **rollback safety**: see the risk table.

Ordering matters and is dictated by the `/srv/src` recovery playbook, which says
capture state before mutating. That is impossible here — `git status` fails in
every broken worktree — so the order is inverted deliberately: **rewrite pointers
only (non-destructive) → capture state → only then consider anything that could
lose work.** Steps 1–6 mutate no file in the working tree, and `container_ref` is
never rewritten (orphan classification is an un-canonicalised exact string
compare against it).

This procedure was validated end to end against a synthetic repository: after
hiding the original main repository, adoption into a fresh bare clone restored
`git status`, `git log` and the branch head, kept tracked file contents, kept the
commit the agent had made, and preserved untracked files.

**Where it runs.** In `ensure_container_exists` for a cluster-placed workspace
(`container.rs:2600`), so a workspace heals the first time it is touched; and in
a bounded startup sweep over cluster-placed workspaces, so the fleet heals
without user interaction. The sweep walks `{shared_root}/workspaces` only, which
naturally excludes the deploy build worktree at
`/srv/src/vibe-kanban-rebuild-cache/build-tree` — that tree is deliberately left
behind between builds and must not be touched.

Adoption is per-worktree best-effort with a truthful aggregate: one repo failing
in a multi-repo workspace does not abort the others or let the workspace report
success. Every outcome — adopted, skipped, failed — is logged at `info!` with the
path and the reason it was selected, *before* acting. Adoption makes no database
writes; the only state it touches is the placement reason it already owns.

It is explicit, coordinator-owned and predictable — never an implicit
remediation triggered as a side effect of an ordinary Git call. Skipped is the
correct outcome, not repair, when the workspace's assigned worker is unreachable:
unreachable means *indeterminate*, not idle.

### 6. Worker-side preflight

The worker cannot repair anything — it has no Git administration role and must
not gain one — but it can refuse to start work on a workspace it cannot use.

In `crates/worker/src/execution.rs`, after `authorize_workspace_path`, probe each
repo directory the worker discovers (`discover_repo_names`, `:479-494`) and reject
the dispatch with a new typed reason when the worktree linkage does not resolve.
This surfaces on the coordinator as a dispatch failure with an actionable message
instead of `fatal: not a git repository` buried in an agent transcript, and it
terminalises the worker job — a failed dispatch that leaves a pending record
contaminates later reconciliation.

The probe is a pure filesystem read (`.git` → `gitdir:` → `commondir`); it adds no
Git dependency to the worker crate.

### 7. Observability

- Placement reasons carry the new failures as plain sentences naming the repo and
  the offending path.
- Provisioning logs the store path, whether it was created or reused, which
  remotes were fetched, and the resolved target-branch OID.
- Adoption logs are explicit about destructive-looking steps (index rebuild) and
  about everything skipped, per the reclamation knowledge-base page.
- The startup sweep's aggregate — repaired / skipped / failed, with reasons —
  is reported through **structured logging and the placement reason**, both of
  which are in-process and already surfaced. No external notification is wired up
  here: `vk-deploy-notify` is a systemd/Nix helper in
  `homelab/modules/vibe-kanban-rebuild.nix`, not a Rust API — it appears nowhere
  in `crates/` — and that module is out of scope. Routing the alarm to it is a
  possible follow-up, not part of this change.

## Acceptance criteria

1. A newly created cluster workspace's `<workspace>/<repo>/.git` points inside
   `shared_root`, and `git status`, `git log`, `git diff` and `git commit` all
   succeed from the worker.
2. An existing broken workspace, when next opened, is adopted in place: tracked
   contents unchanged, untracked files preserved, previously committed agent work
   still reachable, `git status` clean apart from genuine changes.
3. Provisioning a workspace whose worktrees cannot be made portable fails with
   `placement_state = failed` and a reason naming the repository — never `Ready`.
4. A dispatch to a worker whose workspace has an unresolvable worktree is
   rejected with a typed error and a terminalised worker job.
5. `placement_state = local` workspaces are unaffected: same directory, same
   `repo.path`, same unfenced code path (#172's fix stands).
6. Clustering-disabled installations behave exactly as before; no shared store is
   created and no call site changes behaviour.
7. Branch status, rebase, merge, push and PR creation succeed for a cluster
   workspace, operating on the shared store.
8. The verification gates that **can** run in the implementation environment
   pass: `cargo check`, `cargo clippy -- -D warnings` and `cargo test`, all
   `--workspace --exclude vibe-kanban-tauri --all-targets` (webkit2gtk is absent
   from `/nix/store` here, as it is in CI). The gates that **cannot** run here
   are `pnpm run check`, `pnpm run lint` and `pnpm run format`: this workspace is
   on a worker node with no pnpm and no npm, and no package manager to install
   them. Constitution XIV forbids silently skipping a toolchain after reporting
   success, so those three are named as not-run in the completion notes and left
   to CI, which does have them. The change is backend-only, so no generated
   TypeScript moves; that is asserted by inspecting `shared/types.ts` rather than
   by running `generate-types`.

## Testing

- **`shared_repository`**: creation is atomic (a killed clone leaves no valid
  store); remotes are copied; `gc.auto=0` is set; `ensure` is idempotent; a
  missing target branch fails rather than silently proceeding.
- **Adoption**: the reproduction above as a Rust test — main repo hidden, store
  cloned, worktree adopted; asserts branch, log, tracked content, untracked file
  survival, and a clean index. Negative cases: healthy worktree untouched, `.git`
  directory untouched, unresolvable branch refuses to adopt.
- **Portability probe**: unit tests over `.git` file / directory / missing /
  outside-root / symlink-escape shapes.
- **Provisioning**: a non-portable repo makes `create_cluster_workspace`
  transition to `Failed` with a reason, and leaves no `Ready` workspace.
- **Placement regression** (the gap #172 called out): a `local` workspace still
  routes through `ensure_workspace_exists` and `repo.path`.
- **Worker preflight**: dispatch into a workspace with a dangling `.git` is
  rejected and the worker job is terminal.
- **Namespace collision**: two workspaces of the same repo on one store; cleaning
  up one must leave the other's registration and directory intact.
- **Two-node deployment gate** (per `957e-clustered-vibe-k`): create a workspace
  on `think3`, run an agent turn that commits, open a PR from the coordinator,
  disconnect the coordinator, cancel a process group, remove the shared mount, and
  verify worktree integrity after a coordinator restart. Passing unit tests does
  not replace this.

All fixture-based tests use temporary roots. **No test touches the live
worktrees, and no dev server is started on a cluster host** — see the risk table.

The changed crates are added to the CI path filters; a test command in a filtered
job that the changed files do not trigger is not coverage. `crates/git/**` is
already in the `backend` filter (`.github/workflows/test.yml:64`). Genuinely
missing, and holding the store, the namespace containment and the worker
preflight — i.e. most of this change and most of its tests — are
`crates/workspace-manager/**`, `crates/worktree-manager/**`, `crates/worker/**`
and `crates/cluster-protocol/**`. This is a pre-existing hole that this change is
the first to be bitten by.

## Risks

| Risk | Mitigation |
| --- | --- |
| **A rollback to a pre-fix binary while worktrees are already repointed.** The deploy loop health-probes and rolls back automatically. An old binary would find the workspace branch missing in `repo.path`, fall through `try_repair_worktree_in_place`, and reach *destructive recreation*. | Adoption step 7 pushes the branch back into `repo.path`, so an old binary still resolves it and its repair path reattaches non-destructively instead of recreating. The rollout is additionally documented as forward-preferred, and the two-node gate exercises a restart. |
| **Auto-gc on a worker pruning another workspace's registration.** `git gc --auto` fires on ordinary commands and prunes worktrees. | `gc.auto=0`, `gc.autoDetach=false`, `maintenance.auto=false`, `gc.worktreePruneExpire=never` set on the store at creation, before any worktree is added. |
| **Repo-wide `git worktree prune` in `comprehensive_worktree_cleanup`**, whose blast radius goes from per-node to cluster-wide. | Scoped to the worktree being cleaned as part of this change (in scope, not a follow-up). |
| **A misresolved registration in the single shared `worktrees/` namespace.** `find_worktree_git_internal_name` resolves by path correctly, but swallows read failures (`filter_map(.ok())`, `exists()`) into `Ok(None)`, after which the caller falls through to a broader cleanup against a namespace that now holds every workspace of the repo. | Both idioms become `try_exists()` and a propagating `read_dir`, so an unreadable namespace is an error, not an empty one. Adoption uses a workspace-UUID-derived admin name rather than the basename Git would pick. Covered by a test with two workspaces of the same repo. |
| Lock-inode incompatibility between the coordinator's and workers' principals | The export maps every writer to the same storage-side identity (`977:988`, verified on this worker); the store is additionally created setgid with umask `002` before the first write. |
| NFSv3 lock semantics for `*.lock` ref updates | Contention is per-branch and per-worktree; the export is `hard`-mounted and Git's `O_EXCL` create is the same primitive already used for the workspace's own index. Measured clean under 4-way concurrent commits; covered by the two-node gate. |
| Disk growth under the shared root | One object store per repo, not per workspace. 35 TB of 37 TB free, measured. `objects/info/alternates` is explicitly rejected: it stores an absolute path, which is the bug being fixed. |
| Adoption picking the wrong branch tip and losing commits | Adoption refuses when the branch cannot be resolved, proves the object with `cat-file -e`, and never falls back to the target branch. |
| Adoption stealing a branch already checked out by a live workspace | Refuse when the branch is checked out by another worktree of the store. |
| `/srv/src/<repo>` reset by `git-projects-update` before a branch is fetched | The store fetches all `refs/heads/*` from `repo.path` on every `ensure`, and `git reset --hard` does not delete branches, so the window is bounded. Adoption additionally reads the tip from the old common dir. |
| Doubling the meaning of "repo path" | One resolver (`workspace_repo_git_path`) with an explicit list of converted and deliberately-unconverted call sites, applied at *every* child-process boundary (container and PTY), plus a test that a `local` workspace resolves to `repo.path`. No process-global `GIT_DIR`/`GIT_COMMON_DIR` and no mutated global Git config. |
| **Testing the sweeps on this host.** `cleanup_orphan_workspaces` always sweeps the default base dir even with an override, and against a non-matching DB would classify every live worktree as an orphan. In this fleet it is in fact **inert**: `workspace_manager.rs:714-719` early-returns when `allow_reclamation == false`, and `container.rs:1025` passes `!cluster_config.enabled`, so with clustering on it never runs. | Sweep behaviour is tested only against temporary fixture roots. No dev server is started on a cluster host during this task — not because the orphan sweep would fire, but because a second server would contend for the same shared root, the same SQLite administration leases and the same live worktrees this task is repairing. |

## Resolved questions

- **Bare or non-bare store?** Bare. Nothing checks it out, `worktrees/` sits at
  the top level, and there is no working tree for anything to reset.
- **Rewrite `repos.path` to the store instead of adding a resolver?** No. It would
  silently redefine an operator-configured path, break "open in editor" and repo
  search (a bare store has no working tree), and change behaviour for non-cluster
  installs.
- **Should the worker repair worktrees?** No. `957e-clustered-vibe-k` reserves all
  worktree administration for the coordinator; the worker only refuses.
- **Change the homelab module?** No. `/srv/src` stays local build-input storage;
  the shared root, mount and export are already correct, and `{shared_root}` is
  already covered by the units' `ReadWritePaths`, so `repositories/` inherits it.
  This is verified, not assumed.
- **What happens when a workspace moves between local and cluster placement?**
  Nothing — the transition does not exist. `worker_node_id` is immutable after
  reservation and `local` is a terminal state, so a workspace's object store never
  changes. This resolves the bifurcation the knowledge base flags: two stores for
  one repository is acceptable precisely because no workspace ever crosses.
- **How does a branch pushed by hand into `/srv/src/<repo>` reach the store?**
  `ensure` fetches `+refs/heads/*` from `repo.path` on every provisioning, so it
  arrives on the next workspace operation for that repo.
