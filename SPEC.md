# Cluster workspace creation fails with "An internal error occurred"

Task id: `b72a-internal-error-o`

> Constraints distilled from the project knowledge base are in
> [`PRIOR_KNOWLEDGE.md`](PRIOR_KNOWLEDGE.md).

## Symptom

On the coordinator (think2, `vibe.vasandani.dev`), starting a new issue from the
create-mode chat box ("What would you like to work on?") with **Run on:
Automatic placement** returns:

> An internal error occurred. Please try again.

Sometimes instead the request hangs for a long time before the workspace
eventually starts. Reported against release `293f7017` (PR #174, built
2026-08-02T16:44Z), on a two-repository selection (`homelab`, `vibe-kanban`).

## Root cause

`SharedRepositoryStore::ensure` — added by #174 — decides whether the shared
bare store can serve a workspace's **target branch** by asking for one specific
ref (`crates/workspace-manager/src/shared_repository.rs:441-449`):

```rust
fn branch_commit_present(cli: &GitCli, store: &Path, branch: &str) -> Result<bool, WorkspaceError> {
    let reference = format!("refs/heads/{branch}");
    cli.commit_exists(store, &reference)
    ...
}
```

A `target_branch` in this codebase is **not** guaranteed to be a local branch
name. The repo-wide convention, established by `GitService::find_branch`
(`crates/git/src/lib.rs:1410-1425`) and `GitService::check_branch_exists`
(`crates/git/src/lib.rs:1281-1294`), is *local first, then remote*: the string
`origin/main` resolves to `refs/remotes/origin/main`.

And `origin/main` is exactly what the create screen sends. `get_all_branches`
returns remote-tracking branches under their remote-prefixed names, and
`resolveDefaultBranch` (`packages/web-core/src/shared/lib/defaultBranch.ts`)
defaults a newly added repo to the literal string `origin/main` (see
`wiki/create-mode-repo-branch-defaulting.md`). `repo.default_target_branch` is
NULL at registration, so for most repos the built-in `origin/main` default is
what actually applies.

The store is a `git clone --bare` of the coordinator's checkout, so it holds
`refs/heads/main` and has **no** `refs/heads/origin/main` and no
`refs/remotes/origin/main`. Verified on the live share:

```
$ git -C /srv/vibe-kanban-shared/cluster/repositories/b2a286a2-… \
      cat-file -e 'refs/heads/origin/main^{commit}'
fatal: Not a valid object name refs/heads/origin/main^{commit}
$ git -C … cat-file -e 'refs/heads/main^{commit}'      # PRESENT
```

So for a repo left on the default branch, `ensure` takes the slow path every
time and then fails at its closing check (`shared_repository.rs:348-353`):

```rust
if !Self::branch_commit_present(&cli, store, target_branch)? {
    return Err(WorkspaceError::PartialCreation(format!(
        "shared store {} does not resolve target branch '{target_branch}'", …)));
}
```

`PartialCreation` → `LocalContainerService::map_workspace_manager_error`
(`crates/local-deployment/src/container.rs:628`) → `ContainerError::Other` →
`ApiError::Container` → `ErrorInfo::internal("ContainerError")`
(`crates/server/src/error.rs:518`) → the generic message the user sees.

The mirroring fetch that is supposed to top the store up cannot help, because it
only copies the checkout's *local* heads (`shared_repository.rs:324-328`):

```rust
cli.fetch_with_refspec(store, &repo.path.to_string_lossy(), "+refs/heads/*:refs/heads/*")
```

Even if `branch_commit_present` were fixed on its own, provisioning would still
fail one step later: `create_worktree` calls
`GitService::create_branch(store, "vk/…", target_branch)`
(`crates/worktree-manager/src/worktree_manager.rs:274-282`), whose `find_branch`
looks for `refs/remotes/origin/main` **in the store**. The store must actually
carry the remote-tracking refs.

### Why it is intermittent

It is deterministic per branch selection, not random:

- target branch left at the default `origin/main` → always fails;
- target branch changed to a local name (`main`, a `vk/…` branch, or whatever
  `resolveDefaultBranch` fell through to when `origin/main` was absent from the
  list) → succeeds.

That is why the same user sees both outcomes, and why the cluster workspaces
that do exist on the share (`b72a595d`, `19a4a176`, `87949b5a`) all carry local
branch names.

### Why it hangs

Before returning the error, `ensure` does slow work per repository, inside the
HTTP request, with no timeout at any layer (no axum `TimeoutLayer`;
`makeLocalApiRequest` issues a bare `fetch` with no `AbortSignal`):

1. `clone_into_staging` — a full `git clone --bare` of the checkout onto the
   shared NFS root. Measured on this cluster: 0.7 s for homelab's 116 MB, so
   this is not the dominant cost, but it is repeated per repository and on every
   attempt until the store is published.
2. The remotes fallback loop (`shared_repository.rs:334-346`) runs
   `git fetch <origin-url> +refs/heads/origin/main:refs/heads/origin/main`
   against **`https://github.com/…`** — a network round trip guaranteed to fail,
   because no branch named `origin/main` exists upstream. This is the dominant,
   user-visible stall, and it is paid once per repository per attempt.

### Pre-existing defect this exposes

Every failure inside clustered provisioning and dispatch is funnelled through
`ContainerError::Other(anyhow!(…))` and rendered as
"An internal error occurred. Please try again." The one message that identifies
this in a single read —
`shared store … does not resolve target branch 'origin/main'` — never reaches
the operator; the coordinator's journal is the only copy. "Debug the error" was
itself made expensive by this.

## Scope

In scope:

1. Resolve a cluster workspace's target branch in the shared store the way the
   rest of the codebase resolves target branches: local ref first, then
   remote-tracking ref.
2. Make the shared store carry what that resolution needs, by mirroring the
   registered checkout's remote-tracking refs alongside its heads.
3. Stop the remotes-fallback loop from constructing a refspec that cannot
   succeed for a remote-prefixed branch name.
4. Surface actionable messages for shared-store provisioning failures instead of
   the generic internal error.

Out of scope (recorded, not fixed here):

- The absent request timeout on `POST /api/workspaces/start` and the absent
  client-side `AbortSignal`.
- `WorkerClient::endpoint_for`'s positive-only endpoint cache and missing
  connect timeout (an unreachable configured endpoint costs the full 30 s client
  timeout on every cache miss).
- Rolling back the workspace row when provisioning fails, so a failed create
  does not leave a half-built workspace behind.

## Requirements

**R1 — Remote-prefixed target branches resolve in the store.**
`SharedRepositoryStore::ensure(repo, "origin/main")` succeeds for a repository
whose checkout has `refs/remotes/origin/main`, and the resulting store resolves
`origin/main`.

**R2 — Resolution matches the rest of the codebase.** The store's branch
resolution is local-then-remote, identical in outcome to
`GitService::find_branch`. A name that is both a local branch and a remote
branch resolves to the local one, as `find_branch` does.

**R3 — The store carries remote-tracking refs.** `ensure` mirrors both
`refs/heads/*` and `refs/remotes/*` from the registered checkout, so
`GitService::create_branch(store, …, "origin/main")` and `git worktree add`
succeed against the store.

**R4 — No unsatisfiable network fetch.** The remotes fallback must not fetch
`+refs/heads/origin/main:refs/heads/origin/main`. For a remote-prefixed target
branch it fetches the upstream branch under its real upstream name into the
matching remote-tracking ref, or does not run at all.

**R5 — Actionable failure.** When shared-store provisioning fails, the API
response names what failed and for which repository, rather than
"An internal error occurred. Please try again."

**R6 — No behaviour change off the cluster path.** Coordinator-local
(`placement_state = local`) workspaces and installs with clustering disabled are
untouched — they never construct a `SharedRepositoryStore`.

## Verification

- Unit tests in `crates/workspace-manager/src/shared_repository.rs` over real
  temporary git repositories:
  - `ensure` with a remote-prefixed target branch succeeds and the store
    resolves it (R1, R3);
  - a local branch and a remote branch of the same name resolve local-first
    (R2);
  - the store created by `ensure` can create the workspace branch and register a
    worktree from a remote-prefixed base (R3);
  - `ensure` still fails, with its existing message, for a branch that exists
    nowhere (no regression in the closing check).
- `cargo test --workspace`, `cargo clippy -D warnings`, `cargo fmt`.
- Live re-check on the shared store that `origin/main` resolves after the
  change.
