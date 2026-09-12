# Research — `b72a-internal-error-o`

Evidence gathered before designing. Everything here is either read from the
source at `293f7017` (the release the user was running) or measured on the live
cluster from think3.

## 1. Which release produced the report

`/srv/vk-releases/current/release.json` on think3:

```json
{"sha": "293f70174fb33602b4725f6d88aa65c41e5b5fed",
 "build_id": "2596804-1785688580", "built_at": "2026-08-02T16:44:49Z"}
```

`293f7017` is PR #174, "Back cluster worktrees with a repository store on shared
storage", merged 2026-08-02 07:14 and deployed 16:44. The screenshots are
timestamped 21:09 and 21:10 the same day, so the failing code is #174, not the
earlier clustering PRs. This is also why the report is new: #174 introduced
`SharedRepositoryStore` and moved cluster worktree creation off the coordinator's
checkout and onto a bare store.

## 2. The failing check, and why it cannot pass

`crates/workspace-manager/src/shared_repository.rs:441-449`

```rust
fn branch_commit_present(cli: &GitCli, store: &Path, branch: &str) -> Result<bool, WorkspaceError> {
    let reference = format!("refs/heads/{branch}");
    cli.commit_exists(store, &reference)
```

Measured against the live store for `homelab`:

```
$ git -C /srv/vibe-kanban-shared/cluster/repositories/b2a286a2-1831-47e0-b4b8-b55239b49a2a \
      cat-file -e 'refs/heads/origin/main^{commit}'
fatal: Not a valid object name refs/heads/origin/main^{commit}
$ git -C … cat-file -e 'refs/heads/main^{commit}'          # exit 0
$ git -C … for-each-ref --format='%(refname)' 'refs/heads/origin/*'
                                                            # (empty)
$ git -C … for-each-ref 'refs/heads/**' | wc -l
800
```

Both live stores (`b2a286a2…` = homelab, `cdce12c2…` = vibe-kanban) behave
identically. `git clone --bare` copies the source's `refs/heads/*` and creates no
`refs/remotes/*`, so a remote-prefixed name can match nothing.

## 3. Where `origin/main` comes from

`wiki/create-mode-repo-branch-defaulting.md`, the page written by the task that
introduced the default:

> `git::get_all_branches` returns `GitBranch` whose `name` is `origin/main` for
> remote-tracking branches and `main` for local ones … So a default of
> "origin/main" must match the literal string `origin/main`, not `main`.

> `repo.default_target_branch` is NULL at registration … so for most repos the
> built-in `origin/main` default is what actually applies.

`resolveDefaultBranch`'s order is
`configured default -> origin/main -> origin/master -> is_current -> first`.
So the remote-prefixed form is the *dominant* case, not an edge case — which is
why the failure looked intermittent to the user while being deterministic per
branch selection.

The same page argues against fixing it on the frontend: the picker matches by
exact `name` against `get_all_branches`, so stripping the prefix there would
break the match. The backend is the correct side of the seam.

## 4. The convention the fix must match

`crates/git/src/lib.rs:1410-1425`

```rust
pub(crate) fn find_branch<'a>(repo: &'a Repository, branch_name: &str) -> Result<git2::Branch<'a>, GitServiceError> {
    match repo.find_branch(branch_name, BranchType::Local) {
        Ok(branch) => Ok(branch),
        Err(_) => match repo.find_branch(branch_name, BranchType::Remote) { … }
```

`check_branch_exists` (`lib.rs:1281-1294`) is the same shape. This is the rule
`add_repository` validates the user's choice with, and the rule
`GitService::create_branch` resolves the base branch with. Local first, then
remote. The store must not invent a second one (constitution XXI).

## 5. Why fixing only the resolver is not enough

`crates/worktree-manager/src/worktree_manager.rs:274-282` — worktree creation
calls `GitService::create_branch(store, "vk/…", base_branch)`, whose
`find_branch` needs `refs/remotes/origin/main` **in the store**. Broadening
`branch_commit_present` alone would move the failure from `ensure` into
`create_worktree` and change the message, not the outcome. The store has to
actually carry the refs.

Confirmed on a scratch clone of `/srv/src/homelab` on the shared mount:

```
after '+refs/heads/*:refs/heads/*' only:   rev-parse origin/main   -> fails
after adding '+refs/remotes/*:refs/remotes/*': rev-parse origin/main -> ok
                                            git branch vk/x origin/main -> ok
                                            git worktree add … vk/x     -> ok, on vk/x
```

## 6. Where the mirrored refs must come from

`homelab/knowledge-base/cloudflare-access-service-token-live-enablement.md`:

> `git clone /srv/src/homelab /tmp/x && git checkout origin/main` resolves
> `origin/main` to the *clone's* origin — i.e. `/srv/src/homelab`'s **local**
> `main` … not GitHub's main.

So the store must not be given its own `origin` fetch refspec and told to
populate `refs/remotes/origin/*` itself — `configure()` retargets `origin` at the
forge, and the result would be a second notion of `origin/main` with different
freshness. Copying the **checkout's** `refs/remotes/*` verbatim gives the store
exactly the refs the branch picker read, from the same place, and `clone --bare`
leaves nothing there to shadow.

## 7. The unsatisfiable network fetch (the stall)

`shared_repository.rs:334-346` builds
`+refs/heads/{target_branch}:refs/heads/{target_branch}` and fires it at each
non-`vk-registered` remote — for `origin/main` that is
`git fetch https://github.com/davidvasandani/homelab +refs/heads/origin/main:refs/heads/origin/main`,
which cannot succeed because no branch named `origin/main` exists upstream. It is
paid once per repository per attempt, on the create request, with no timeout at
any layer: there is no axum `TimeoutLayer`, and `makeLocalApiRequest` issues a
bare `fetch` with no `AbortSignal`.

Measured, so the *other* candidate is ruled out rather than assumed: the git work
itself is not slow on this mount. `git clone --bare /srv/src/homelab` (116 MB)
onto the NFS share took **0.66 s**, and `git worktree add` **2.5 s**. So the
5-minute administration lease is not being outrun, and the stall is the network
fetch, not shared storage.

## 8. How the message is lost

```
WorkspaceError::PartialCreation
  → map_workspace_manager_error            local-deployment/src/container.rs:628
  → ContainerError::Other(anyhow!(msg))
  → ApiError::Container                    server/src/error.rs:164 (catch-all)
  → ErrorInfo::internal("ContainerError")  server/src/error.rs:518
  → "An internal error occurred. Please try again."
```

`error.rs:550` logs via `tracing::error!` only when the status is a server error,
so the real message exists exactly once, in the coordinator's journal — which is
on think2 and not reachable from a worker. That is why this diagnosis had to be
built from filesystem forensics.

`ApiError::Worktree` (`error.rs:525-529`) is the in-repo precedent for the
remedy: a 500 that still carries a real message.

## 9. Alternatives considered and rejected

| Option | Rejected because |
| --- | --- |
| Normalise `origin/main` → `main` before it reaches the store | Silently substitutes a different branch. The two can and do diverge, and the user picked the remote one. |
| Fix the default in the create-mode picker | The picker matches by exact `name` against `get_all_branches`, which returns remote-prefixed names; the knowledge base records this as the wrong side of the seam. |
| Give the store its own `origin` fetch refspec and let it fetch | Reintroduces the "clone's own origin" hazard from §6; also puts a forge round trip on the create path. |
| Mirror with `--prune` to keep the store tidy | `refs/remotes/*` in the store is repository-wide across every workspace and node; a prune's blast radius is not this task's to take (constitution XX). |
| Make every `ContainerError::Other` render its message | Unbounded blast radius over every internal failure in the product, including ones whose text is not written for users. |
