# Research: Portable Git worktrees for cluster-placed workspaces

## Root cause, established by reproduction

Observed on worker `think-cluster`, in this very workspace. A sweep of the live
share found 15 worktrees across 9 cluster workspace directories: 15 broken,
0 resolving. The failure rate among cluster placements is total.

```
$ cat {shared_root}/workspaces/19a4a176-…/vibe-kanban/.git
gitdir: /srv/src/vibe-kanban/.git/worktrees/vibe-kanban8
$ ls /srv/src/vibe-kanban
ls: cannot access '/srv/src/vibe-kanban': No such file or directory
$ git -C {shared_root}/workspaces/19a4a176-…/vibe-kanban status
fatal: not a git repository: (null)
```

The sibling `homelab` worktree shows the more dangerous shape: `/srv/src/homelab`
*does* exist on this worker (a different clone managed by `services.gitProjects`)
but has no `.git/worktrees/` directory. Had the registration existed, the
workspace would have bound silently to an unrelated repository. This is the
concrete case behind constitution XX's "a same-named local directory is not the
target".

`specs/vk/957e-clustered-vibe-k/` never addressed where the *source* repository
lives. `research.md:72` there rejects "let workers create worktrees" because it
"races shared `.git/worktrees` metadata" — an argument that presumes the metadata
is shared. It is not.

## Measurements taken on the production export

Both on `172.16.0.99:/var/nfs/shared/VibeKanban` (NFSv3, `hard`), in a scratch
directory since removed.

1. **Concurrent multi-worktree writes.** One bare store, four linked worktrees,
   15 sequential commits each, run concurrently: 60 commits, five correct branch
   heads, clean `git fsck`. Evidence the shape works on this storage — not proof;
   the two-node gate remains the authority.
2. **In-place adoption.** A worktree whose main repository was hidden was
   re-linked into a fresh bare clone by writing `worktrees/<n>/{commondir,gitdir,HEAD}`
   and the worktree's `.git`, then `git worktree repair`, then `git reset`.
   Result: branch, `git log`, tracked contents, the commit made before the break,
   and untracked files all intact; index clean. Without the final `git reset` the
   worktree reports every tracked file as both deleted and untracked — the
   absent-index artefact.

Capacity: 35 TB free of 37 TB. One object store per repository, not per
workspace.

## Decisions

### D1 — A bare store per repository under the shared root

`{shared_root}/repositories/{repo_id}`, the path
`SharedWorkspacePaths::repository_dir` already computes. Bare because nothing
checks it out, `worktrees/` sits at the top level, and there is no working tree
for anything to reset.

### D2 — A resolver, not a rewrite of `repos.path`

Rejected rewriting the registered path to the store: it would silently redefine
an operator-configured value, break "open in editor" and repository search (a
bare store has no working tree), and change behaviour for non-cluster installs.
One resolver with an explicit list of converted and deliberately-unconverted call
sites is auditable; a DB rewrite is not reversible.

### D3 — Clone outside the lease, publish inside it

The repository administration lease is a bounded SQLite lease. A multi-minute
clone of a large repository could outlive it, silently unfencing the operation
the lease exists to fence. Clone into a per-attempt staging directory outside the
lease; hold the lease only for verify → configure → `rename(2)` → fetch.

### D4 — Repair, never recreate

The existing worktrees hold agent edits, `node_modules`, build caches, and
commits reachable only from the coordinator's checkout. The `/srv/src` recovery
playbook says capture state before mutating — impossible here, because `git
status` fails in every broken worktree. The order is therefore inverted
deliberately: rewrite pointers (non-destructive) → capture → only then consider
anything that could lose work.

## Alternatives rejected

| Alternative | Why not |
| --- | --- |
| **Export `/srv/src` over NFS too** | The NFS server is a NAS appliance (`172.16.0.99`), not the coordinator, so the coordinator would have to run `nfsd`. Breaks the single-writable-shared-root model mount health is built on, and collides with each worker's own `/srv/src`. |
| **Rewrite the `gitdir:` pointer per node at dispatch** | The object store still lives on coordinator-local disk. There is nothing to point at. |
| **A full clone per workspace** | Correct but expensive: no hardlinks across the NFS boundary, so every workspace pays a full object-store copy — 15 today, and one more for every cluster workspace ever created, against one store per repository under D1. Also breaks `check_branch_exists(&repo.path, …)`, `get_base_commit(&repo.path, …)` and every PR/merge call site, because the branch would exist in no store the coordinator queries. |
| **`objects/info/alternates` pointing at the local repository** | Stores an absolute path — precisely the bug being fixed. Would only be admissible if the referenced path were itself under the shared root, at which point D1 is simpler. |
| **Let workers create their own worktrees** | Rejected already in `957e-clustered-vibe-k/research.md:72`, and it would put administration on N nodes against one shared namespace. |
| **Documentation only** (`docs/self-hosting/clustered-workers.mdx:34` already tells operators to create `repositories/`) | Leaves an unenforced convention whose violation is silent and costs a turn's work. The deployment cannot follow it anyway: `/srv/src/vibe-kanban` is `forceSync` and declared build-input-only. |
| **A forward-only rollout gate** | Would disable the deploy loop's automatic rollback — a larger safety mechanism than the degradation it prevents. See `clarifications.md` C4. |

## Dependencies

**No new dependency, top-level or otherwise.** Everything is built from crates
already in the workspace: `git2`/`GitCli` (`crates/git`), `tokio::fs`,
`RepositoryAdminLockManager` (`crates/worktree-manager`), and the existing
placement state machine. Constitution "Constraints" requires new dependencies to
be recorded here; there are none to record.

## Pre-existing defects this change is obliged to fix

Both are recorded as known-unfixed in
`docs/knowledge-base/workspace-directory-reclamation.md`. Neither is optional
here, because consolidating every workspace of a repository onto one store raises
their blast radius from per-node to cluster-wide:

- `comprehensive_worktree_cleanup` ends with a repo-wide `git worktree prune`.
  The same operation killed a production build on 2026-07-05 when it walked
  foreign-owned registrations (`wiki/self-hosted-deployment.md`).
- Admin directory names are derived by Git from the worktree path's *basename*,
  so one store's `worktrees/` namespace now holds one entry per live workspace
  instead of a handful per node. VK's own resolution is *not* basename-derived
  and must not be rewritten as if it were —
  `force_cleanup_worktree_metadata` (`worktree_manager.rs:755-777`) goes through
  `find_worktree_git_internal_name` (`:573-610`), which compares canonicalised
  paths. What does not carry over is that function's error handling: it uses
  `read_dir(...).filter_map(|entry| entry.ok())` (`:583-585`) and
  `gitdir_path.exists()` (`:599`), so an NFS read failure returns `Ok(None)` and
  the caller falls through to a broader cleanup against the consolidated
  namespace.
