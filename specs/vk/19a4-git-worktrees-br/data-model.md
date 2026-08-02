# Data Model: Portable Git worktrees for cluster-placed workspaces

**No schema migration.** Every existing table keeps its shape. What changes is
the filesystem layout under the shared root and how an existing column
(`repos.path`) is interpreted for cluster-placed workspaces.

## Filesystem layout under `{shared_root}`

`{shared_root}` = `/srv/vibe-kanban-shared/cluster` (`VK_CLUSTER_SHARED_ROOT`),
identical on every node. Paths are derived from UUIDs, never user-controlled
names (`SharedWorkspacePaths`, `crates/workspace-manager/src/workspace_manager.rs:26-83`).

```
{shared_root}/
  repositories/
    {repo_id}/                       ← NEW: bare store, was an empty directory
      HEAD  objects/  refs/  config
      worktrees/
        {name}/                      ← one per cluster workspace of this repo
          commondir   -> "../.."
          gitdir      -> {shared_root}/workspaces/{workspace_id}/{repo_name}/.git
          HEAD        -> "ref: refs/heads/{workspace.branch}"
          index, ORIG_HEAD, logs/    ← written by whichever NODE runs git here
    .{repo_id}.incoming/             ← per-attempt staging, renamed into place
  workspaces/
    {workspace_id}/
      {repo_name}/
        .git                         ← "gitdir: {shared_root}/repositories/{repo_id}/worktrees/{name}"
  execution-logs/
  .coordinator-probes/
```

Before this change, `{workspace_id}/{repo_name}/.git` read
`gitdir: /srv/src/{repo_name}/.git/worktrees/{name}` — a coordinator-local path.
That single line is the defect.

### Entity: shared repository store

| Property | Contract |
| --- | --- |
| Location | `{shared_root}/repositories/{repo_id}`, derived, never configured |
| Kind | Bare repository |
| Identity | The repository's UUID; the display name is never in the path |
| Remotes | Copied from `repos.path`; `origin` retargeted to the real forge, not to the local checkout that `git clone --bare` would point it at |
| Config | `gc.auto=0`, `gc.autoDetach=false`, `maintenance.auto=false`, `gc.worktreePruneExpire=never`, `core.logAllRefUpdates=true` — written **before** the first worktree is added |
| Ownership | Directory created setgid, group-owned, umask `002`, before the first object is written |
| Creation | Clone into `.{repo_id}.incoming` outside the admin lease; verify, configure and `rename(2)` under it |
| Writers | Administration (`worktree add/remove/prune`, shared branch deletion) — coordinator only, fenced. Ordinary commands — every node, including the per-worktree `index`/`HEAD`/`logs` |
| Lifetime | Retained while any workspace references the repository; reclaimed only when the repo record is gone and nothing references it; any error retains (FR-29) |

### Entity: worktree linkage (derived, not stored)

Computed by probing the filesystem; never persisted, so it cannot go stale.

| Field | Meaning |
| --- | --- |
| `worktree_path` | The directory being probed |
| `pointer` | The `gitdir:` target read from `{worktree}/.git` |
| `common_dir` | Resolved via the pointer's `commondir` |
| `back_pointer` | `{common_dir}/worktrees/{name}/gitdir`, which must name `{worktree}/.git` |
| `status` | `Portable` \| `NotApplicable` \| `Dangling { target }` \| `OutsideSharedRoot { common_dir }` \| `Indeterminate { reason }` |

`Indeterminate` is load-bearing: a stat or read failure on NFS is not evidence of
either health or breakage, and must never collapse into `Portable`.

## Existing tables — unchanged, reinterpreted

### `repos`

| Column | Change |
| --- | --- |
| `path` | **Unchanged.** Still the operator-registered coordinator-local checkout. Still used for "open in editor", repository search, repo-level branch and remote listing, and as the seed and push-back sink for the store. No longer used for the worktree administration of a cluster-placed workspace. |

The store path is *derived* from `repos.id`, so no column is added. Adding one
would create a second representation of the same fact and let the two disagree.

### `workspaces` / `workspace_placements`

Unchanged. `placement_state` is already the authority for which store a workspace
belongs to:

`WorkspacePlacementState` has **six** variants
(`crates/db/src/models/workspace.rs:105-113`):

| `placement_state` | Backing repository |
| --- | --- |
| `local` | `repos.path` — exactly as before this change |
| `reserved`, `provisioning`, `ready`, `failed`, `cleaning` | `{shared_root}/repositories/{repo_id}` |

`cleaning` is in the second group deliberately: cleanup of a cluster workspace
must run worktree administration against the store the worktree was created
from, and falling through to `repos.path` there is exactly the FR-24 failure.
The resolver matches this enum **exhaustively, with no wildcard arm**, so a
seventh variant is a compile error rather than a silent fallthrough to
`repos.path`.

Transitions remain monotonic (`reserved → provisioning → ready\|failed`) and
`worker_node_id` remains immutable after reservation, so **a workspace never
changes store**. That is what makes two stores per repository safe: no workspace
ever crosses between them.

`container_ref` is **never rewritten** by this change. Orphan classification is
an un-canonicalised exact string compare against it
(`docs/knowledge-base/workspace-directory-reclamation.md`), so rewriting it would
make live workspaces look abandoned.

### `repository_admin_locks`

Unchanged, but its scope widens: it now fences store provisioning and adoption in
addition to `worktree add`/`remove`. The lock key is already the repository's
*common directory* (`canonical_lock_key`,
`crates/worktree-manager/src/worktree_manager.rs:178-189`), which resolves to the
store for a cluster workspace — so a repository registered as one of its own
worktrees still maps to a single lock.

## In-memory types

| Type | Change |
| --- | --- |
| `RepoWorkspaceInput` (`workspace_manager.rs:95-107`) | Gains `git_path: PathBuf`. `::new` keeps today's behaviour (`git_path = repo.path`); `::shared` supplies the store. |
| `RepoWorktree` (`workspace_manager.rs:136-141`) | Shape unchanged, but its **construction is not**: `workspace_manager.rs:444` reads `source_repo_path: input.repo.path.clone()`, so it must become `input.git_path.clone()`. Without that edit it carries the coordinator-local store into rollback and cleanup for every cluster workspace. With it, the value propagates correctly to `:695`. |
| `WorkspaceError` | Gains `WorktreeNotPortable { repo, common_dir }`. |
| `SharedWorkspacePaths` | Unchanged. `repository_dir` stops being dead code. |
