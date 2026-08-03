# Prior knowledge — `b72a-internal-error-o`

Task: clustered workspace creation returns "An internal error occurred. Please
try again." Distilled from the knowledge bases this workspace can reach:

- `vibe-kanban/wiki/` (19 pages + `INDEX.md`)
- `vibe-kanban/docs/knowledge-base/` (22 pages + `INDEX.md`)
- `homelab/knowledge-base/` (12 pages + `index.md`)

Constraints marked **[H]** are hazards that change the design rather than
merely constrain it.

## The page that identifies the bug

### `wiki/create-mode-repo-branch-defaulting.md`

**[H] Target branch names carry the remote prefix.** Verbatim:

> **Branch names carry the remote prefix.** `repoApi.getBranches` →
> `GET /api/repos/{id}/branches` → `git::get_all_branches` returns `GitBranch`
> whose `name` is `origin/main` for remote-tracking branches and `main` for
> local ones (`is_remote` distinguishes them). So a default of "origin/main"
> must match the literal string `origin/main`, not `main`.

**[H] The default that actually applies is `origin/main`.** The
`resolveDefaultBranch` fallback order is
`configured default_target_branch -> origin/main -> origin/master ->
the is_current branch -> the first branch -> null`, and:

> **`repo.default_target_branch` is NULL at registration.** It is only set via
> repo settings (`ReposSettingsSection`), so for most repos the built-in
> `origin/main` default is what actually applies.

Consequence for this task: any code that consumes a `target_branch` must
resolve it local-first-then-remote. `SharedRepositoryStore::branch_commit_present`
asks only for `refs/heads/{branch}`, so it can never satisfy the default. This
page is the reason the failure looks intermittent — it is deterministic per
branch selection.

Also recorded there, and relevant to *not* over-fixing: there is a dormant
second selector (`useRepoBranchSelection.ts` / `RepoBranchSelector.tsx`) with
divergent defaults and **no importers**. Do not touch it; reconciling it is a
separate task.

## Rules the fix must obey

### `docs/knowledge-base/clustered-workspace-execution.md`

Tagged `957e-clustered-vibe-k`, `19a4-git-worktrees-br` — the page written by
the change that introduced the defective code. Four rules from
"A worktree is only as portable as the repository behind it" bind this task:

> - **Assert structure, not spelling.** …
> - **Existence proves nothing.** A same-named local directory is not the
>   repository. On these hosts `/srv/src/<repo>` exists on workers too, holding
>   a different clone — a resolver that accepts it binds the workspace to
>   unrelated history.
> - **Prove the objects.** `git rev-parse` echoes any well-formed 40-hex string
>   whether or not the repository holds it. Use `git cat-file -e <rev>^{commit}`
>   before treating a branch as present.
> - **Check level-triggered.** …

**[H] "Prove the objects" constrains the fix directly.** Broadening resolution
from `refs/heads/{b}` to also accept `refs/remotes/{b}` must keep using
`commit_exists` (`cat-file -e`), not `rev-parse`. Do not "simplify" the new
resolver to `rev-parse --verify`.

**[H] Store configuration is clone-time, not after-the-fact.**

> create the store `core.sharedRepository=group` **at clone time** (setting it
> afterwards leaves every object and directory git already created at the
> cloning process's umask), and disable automatic maintenance before the first
> worktree is registered. `git gc --auto` fires opportunistically on ordinary
> commands and prunes worktrees, so without `gc.auto=0` and
> `gc.worktreePruneExpire=never` a routine `git status` on a worker can
> unregister a different workspace.

So the fix must not reorder `configure()` relative to the clone/rename, and must
not add refs to the store before `core.sharedRepository=group` is in effect.

**Single-writer administration.** Only the coordinator may add/remove/prune
worktrees or delete shared branches, serialized per repository with *fenced*
ownership. The extra ref mirroring therefore belongs inside the existing
administration lease in `publish_and_fetch`, not in a new unfenced code path.

**[H] Mirror additively; never prune.** "When a namespace is consolidated,
re-derive the blast radius of everything that touches it rather than inheriting
the old conclusion." `refs/remotes/*` in the store is repository-wide and shared
by every workspace of that repo on every node, so the mirroring fetch must not
carry `--prune` / `+refs/remotes/*:refs/remotes/*` semantics that delete. A
force-update refspec without `--prune` is additive; keep it that way.

### `homelab/knowledge-base/cloudflare-access-service-token-live-enablement.md`

**[H] `origin/main` in a derived clone can mean the wrong thing.** Verbatim:

> **Gotcha that ate a full apply cycle:** `git clone /srv/src/homelab /tmp/x &&
> git checkout origin/main` resolves `origin/main` to the *clone's* origin —
> i.e. `/srv/src/homelab`'s **local** `main`, which git-projects may not have
> advanced yet — not GitHub's main.

This is the trap the fix must not fall into, and it decides *how* the store
learns about `origin/main`:

- **Wrong:** give the store its own `origin` fetch refspec and let it populate
  `refs/remotes/origin/*` from the clone source. `configure()` retargets
  `origin` at the forge, so that would be a second, differently-fresh notion of
  `origin/main`.
- **Right (chosen):** copy the *registered checkout's* `refs/remotes/*`
  verbatim. `git clone --bare` puts the source's branches in `refs/heads/*` and
  creates no `refs/remotes/*` at all, so there is nothing stale to shadow. The
  store then means exactly what the branch picker meant when it offered
  `origin/main` — both read the same checkout — and is as fresh as that
  checkout's last fetch, no more and no less.

### `docs/knowledge-base/workspace-directory-reclamation.md`

**[H] "I could not tell" must not become "there is nothing here".**

> `Path::exists()` returns `false` for both "absent" and "stat failed" — use
> `try_exists()`, which distinguishes them. A git probe that errors is not a
> clean repo.

and

> the fail-safe direction is **not** consistent across the codebase … When
> adding a new decision, match the safe sibling.

For this task the safe sibling is the existing `commit_exists`, which already
treats `GitCliError::CommandFailed` as "absent" and propagates every other
error. The new local-then-remote resolver must reuse it rather than inventing a
second probe with a different fail direction — the two ref forms must fail the
same way as one another and as the code they replace.

### `wiki/browser-session-control-arbiter.md` — how errors reach the UI

> `ApiError` responses carry only a message string.

The typed serde-tagged payload that page describes exists because MCP tools and
the frontend *parse* browser-session errors back. Nothing parses a provisioning
failure; a human reads it. So the right precedent here is the plainer one
already in `error.rs` — `ApiError::Worktree`, rendered as
`with_status(INTERNAL_SERVER_ERROR, "WorktreeError", "Worktree operation failed:
{err}")` — a 500 that still carries a real message. Follow that shape and leave
the global envelope alone.

**Failed dispatch must terminalise its job.** Recorded as a rule; relevant to
the out-of-scope note about leaving half-built workspaces behind.

### `docs/knowledge-base/interrupted-worktree-recovery.md` and the repair rules

"Repair a broken worktree; never recreate it", and *refuse rather than guess*:
refuse when the branch's commits cannot be proven present. The fix must not
weaken `adopt()`'s refusal — broadening branch resolution changes what counts as
"present", so `adopt` must keep proving the *workspace* branch (always a local
`vk/…` name), and the change must not make a remote-tracking ref look like a
valid adoption target.

## Deployment / operations

### `wiki/self-hosted-deployment.md` and `homelab/modules/vibe-kanban-rebuild.nix`

- Merging to `main` is sufficient to ship: think2 (`clusterRole = "coordinator"`)
  polls the repo, builds a pinned worktree, publishes an immutable release under
  `/srv/vk-releases/build-<id>` with a self-describing `release.json.sha`, flips
  `current` atomically, restarts the services and health-gates the result.
- `workerEndpoints` on think2 lists think3/think4; the coordinator pushes
  `current` to them, so a merged fix reaches all three nodes without manual
  steps. Confirmed live: `/srv/vk-releases/current/release.json` on think3 reads
  `{"sha": "293f70174fb…", "built_at": "2026-08-02T16:44:49Z"}` — i.e. the
  failing release is #174, which is what the user was running at 21:09.
- **[H]** Services must not run from the source checkout; do not add anything
  that writes into `/srv/src/<repo>` on the deploy host beyond what already
  exists (`mirror_branch_back`'s courtesy push is pre-existing and stays
  best-effort).

### `homelab/knowledge-base/cloudflare-access-service-token-live-enablement.md`

Only tangentially relevant, but records that the homelab CI apply job is
unpinned and its preflight curls vibe-kanban's Caddy on think2 — a reminder that
this cluster's nodes are not interchangeable. Not affected by this change.

## Testing conventions

- Rust unit tests live beside the code in `#[cfg(test)] mod tests`;
  `crates/workspace-manager/src/shared_repository.rs` already has a suite that
  builds **real** temporary git repositories via `seed_repo` / `repo_record` /
  `store_for`, and `crates/worktree-manager/src/worktree_manager.rs` shows the
  in-memory `repository_admin_locks` pool pattern (`max_connections(1)` on
  `sqlite::memory:` plus a hand-written `CREATE TABLE`) needed to exercise the
  administration lease. Reuse both rather than inventing new harnesses.
- `seed_repo` sets `user.email`/`user.name` locally because *CI and this
  cluster's worker accounts have no global git identity* — any new fixture repo
  must do the same.
- Verification gate: `pnpm run check`, `cargo test --workspace`,
  `cargo clippy -D warnings`, `pnpm run format`. #174 added
  `crates/{workspace-manager,worktree-manager,worker,cluster-protocol}` to the
  CI backend path filter, so changes in these crates now actually trigger the
  backend job.
- **[H] A repro that also passes before the fix proves nothing**
  (`workspace-directory-reclamation.md`): run the new tests against unmodified
  `main` and confirm they fail.
- **[H] Do not run a dev server to verify on this host** — but not for the
  reason `workspace-directory-reclamation.md` gives. That page's orphan-sweep
  hazard is **inert here**: `container.rs:1153` calls
  `cleanup_orphan_workspaces(!container.cluster_config.enabled)` and
  `workspace_manager.rs:778` early-returns when `allow_reclamation` is false, so
  with clustering enabled the sweep never reclaims. Verified in the source, not
  inherited. The real reason is contention: a second server on this node would
  compete for the same shared root, the same SQLite repository administration
  leases, and the same live worktrees. Verify with unit tests over temporary
  repositories and read-only probes of the live store instead. (Task 19a4
  reached the same conclusion; recording it here so the next reader does not
  re-derive it from the more alarming KB page.)
- `clustered-workspace-execution.md` is explicit that unit tests are not the
  whole gate: "Passing local tests does not replace that deployment gate."
  The deployment exercise is the coordinator's, post-merge.

## Nothing found on

No page covers how `ApiError` collapses cluster failures into the generic
"An internal error occurred" message, or the ergonomics of error surfacing for
clustered provisioning. That gap is why this task's diagnosis was expensive and
is worth recording at stage 12.
