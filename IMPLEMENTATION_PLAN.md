# Implementation Plan — `b72a-internal-error-o`

Fix clustered workspace creation failing with "An internal error occurred".
Design: [`SPEC.md`](SPEC.md). Constraints:
[`PRIOR_KNOWLEDGE.md`](PRIOR_KNOWLEDGE.md).

## Before you start

This workspace runs on a **worker** node (think3), not the coordinator. The
coordinator's journal is not reachable from here, so the diagnosis was built
from the deployed release marker (`/srv/vk-releases/current/release.json` →
`293f7017`), the live shared store under
`/srv/vibe-kanban-shared/cluster/repositories/`, and the source. A Rust
toolchain is available at `~/.cargo/bin` (`nightly-2025-12-04`); add it to
`PATH`. `cargo build --workspace --tests` passes on `main` before any change —
that is the baseline.

Reproduction, without building anything (already run):

```
$ git -C /srv/vibe-kanban-shared/cluster/repositories/b2a286a2-… \
      cat-file -e 'refs/heads/origin/main^{commit}'    # fatal: Not a valid object name
$ git -C … cat-file -e 'refs/heads/main^{commit}'      # PRESENT
```

and, on a scratch clone, that adding `+refs/remotes/*:refs/remotes/*` to the
mirroring fetch makes `origin/main` resolve, `git branch vk/x origin/main`
succeed, and `git worktree add` produce a worktree on `vk/x`.

## Step 1 — `GitCli`: fetch more than one refspec in one invocation

`crates/git/src/cli.rs`

- Add `fetch_with_refspecs(&self, repo_path, remote_url, refspecs: &[&str])`,
  carrying the existing `GIT_TERMINAL_PROMPT=0` env and
  `classify_cli_error` handling verbatim.
- Reimplement `fetch_with_refspec` as a one-element delegation so its three
  existing callers (`git/src/lib.rs:1567`, two tests in
  `git/tests/git_ops_safety.rs`) are unaffected.

Why one invocation rather than two calls: the mirroring fetch runs inside the
repository administration lease, and each `git fetch` is a process spawn plus a
connection to the source.

## Step 2 — resolve a target branch the way the rest of the codebase does

`crates/workspace-manager/src/shared_repository.rs`

- Add `fn resolved_branch_ref(cli, store, branch) -> Result<Option<String>, WorkspaceError>`:
  try `refs/heads/{branch}`, then `refs/remotes/{branch}`, returning the first
  whose commit is **proven present** with `commit_exists` (`cat-file -e`) — per
  the "Prove the objects" rule in `PRIOR_KNOWLEDGE.md`. Local first, so a name
  that is both a local and a remote branch resolves local, matching
  `GitService::find_branch` (`crates/git/src/lib.rs:1410-1425`).
- Reimplement `branch_commit_present` on top of it. Every existing caller
  (`ensure`, `store_resolves`, `adopt`, `mirror_branch_back`) keeps its current
  signature and semantics for local branch names.

## Step 3 — mirror the checkout's remote-tracking refs into the store

`crates/workspace-manager/src/shared_repository.rs`, `publish_and_fetch`

- Replace the single `+refs/heads/*:refs/heads/*` fetch with
  `["+refs/heads/*:refs/heads/*", "+refs/remotes/*:refs/remotes/*"]`.
- Keep it best-effort (the existing `if let Err(e) = … { debug!(…) }`), and keep
  it inside the lease, after `configure()` and the rename — so
  `core.sharedRepository=group` is already in effect for every ref file the
  fetch writes, per `PRIOR_KNOWLEDGE.md`.

This is what makes `GitService::create_branch(store, "vk/…", "origin/main")` and
the subsequent `git worktree add` work; Step 2 alone would move the failure one
frame later.

## Step 4 — make the remotes fallback fetch a refspec that can succeed

`crates/workspace-manager/src/shared_repository.rs`, `publish_and_fetch`

- Extract `fn fallback_refspec(remote_name: &str, target_branch: &str) -> Option<String>`:
  - `target_branch` starting with `"{remote_name}/"` → fetch the upstream branch
    under its real name into the matching remote-tracking ref:
    `+refs/heads/{rest}:refs/remotes/{target_branch}`;
  - otherwise → the current local-to-local form,
    `+refs/heads/{target_branch}:refs/heads/{target_branch}`.
- Break out of the remotes loop only when the branch **actually resolves**
  afterwards, not on `fetch(...).is_ok()`. A zero exit is not evidence that the
  ref we need now exists.

Today this loop always builds `+refs/heads/origin/main:refs/heads/origin/main`
and fires it at `https://github.com/…`; that guaranteed-to-fail network round
trip, once per repository, is the user-visible stall.

## Step 5 — stop the failure surfacing as "An internal error occurred"

- `crates/workspace-manager/src/workspace_manager.rs`: add
  `WorkspaceError::SharedStore { repo_name: String, detail: String }`, whose
  `Display` names the repository and what failed.
- `crates/workspace-manager/src/shared_repository.rs`: return it (instead of
  `PartialCreation`) from `ensure`'s closing "does not resolve target branch"
  check, and include the branch name.
- `crates/services/src/services/container.rs`: add
  `ContainerError::SharedStore(String)` with `#[error("{0}")]`.
- `crates/local-deployment/src/container.rs`: `map_workspace_manager_error` maps
  `WorkspaceError::SharedStore` → `ContainerError::SharedStore`.
- `crates/server/src/error.rs`: add `ApiError::ClusterProvisioning(String)`;
  extend `From<ContainerError>` with an explicit arm **before** the `other =>`
  catch-all; render it as
  `ErrorInfo::with_status(INTERNAL_SERVER_ERROR, "ClusterProvisioningError", msg)`
  — the same "500 with a real message" shape `ApiError::Worktree` already uses,
  so it is still logged by the `is_server_error()` branch.

Blast radius is deliberately one error path: only the shared-store failure
changes what the user sees. Everything else keeps its current mapping.

## Step 6 — tests

`crates/workspace-manager/src/shared_repository.rs`, `mod tests`

Reuse `seed_repo`, `repo_record`; add a `store_with_locks(shared_root)` helper
built on the `worktree_manager` in-memory pool pattern (`max_connections(1)` on
`sqlite::memory:` plus the `CREATE TABLE repository_admin_locks` statement),
because `ensure` — unlike `adopt` — does take the lease.

Fixture: a "checkout" whose `refs/remotes/origin/main` exists, built by cloning
a seed repo and fetching `+refs/heads/main:refs/remotes/origin/main`, so the
fixture mirrors what `/srv/src/<repo>` actually looks like.

1. `ensure_provisions_a_remote_prefixed_target_branch` — `ensure(repo, "origin/main")`
   succeeds; the store resolves `origin/main`; `GitService::create_branch(store,
   "vk/x", "origin/main")` and `worktree_add` then succeed. (R1, R3)
2. `branch_resolution_prefers_a_local_branch_over_a_remote_one` — with both
   `refs/heads/shared` and `refs/remotes/origin/shared` present at different
   commits, `resolved_branch_ref` returns the local ref. (R2)
3. `fallback_refspec_targets_the_remote_tracking_namespace` — pure unit test on
   the helper: `("origin", "origin/main")` →
   `+refs/heads/main:refs/remotes/origin/main`; `("origin", "main")` →
   `+refs/heads/main:refs/heads/main`. (R4)
4. `ensure_still_refuses_a_branch_that_exists_nowhere` — asserts the closing
   check still fires, and now as `WorkspaceError::SharedStore` naming the repo
   and branch. (R5, no regression)

The existing `commit_presence_is_proven_not_assumed` test stays green unchanged
and pins the local-branch behaviour.

## Step 7 — verify

- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `pnpm run format` (`cargo fmt` + prettier), then `pnpm run check` if pnpm is
  usable on this node; note it in the PR if it is not (as #174 did).
- Live re-check on the shared store that `origin/main` resolves once the fix is
  deployed.

## Not doing (recorded in `SPEC.md`)

- Request/client timeouts on `POST /api/workspaces/start`.
- `WorkerClient::endpoint_for`'s positive-only cache and missing connect
  timeout.
- Rolling back the workspace row when provisioning fails.

Each is a real defect on the same request path, but none of them causes this
report, and folding them in would make the change unreviewable.
