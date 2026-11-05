# Implementation Plan: Shared cluster store keeps a local base branch fresh before serving it

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context

Rust, `crates/git` (process-shelled `git` wrapper, `GitCli`) and
`crates/workspace-manager` (`SharedRepositoryStore`, the bare-clone-per-repo
store backing clustered workspaces). No new dependency, no schema change, no
API surface change — this is entirely inside one existing async method
(`SharedRepositoryStore::ensure`) and one existing low-level git helper type
(`GitCli`). `sqlx`/`tokio` test harness conventions already used by this
file's `mod tests` apply unchanged.

## Architecture & Approach

FR-1–FR-5 map onto a single new private method,
`SharedRepositoryStore::fast_forward_local_head`, inserted into the existing
cheap path of `ensure` (`crates/workspace-manager/src/shared_repository.rs`,
around line 89), right after `store_resolves` returns `true` and before
`mirror_branch_back`:

1. Resolve the store's own `origin` remote URL (`GitCli::get_remote_url`).
   Absent ⇒ FR-2's "no remote" case, no-op.
2. Fetch `+refs/heads/<branch>:refs/remotes/origin/<branch>` from that URL
   (`GitCli::fetch_with_refspec`, following the same
   `GitHubCredentials::default()` pattern `publish_and_fetch`'s own fallback
   fetch in this file already uses). A fetch failure ⇒ FR-2's "unreachable"
   case: log and no-op, never propagate as an error.
3. Resolve both tips with the existing `GitCli::resolve_commit`. Equal ⇒
   FR-4, no-op. Local strictly ahead/behind/diverged is distinguished with a
   new primitive, `GitCli::is_ancestor` (added to `crates/git/src/cli.rs`
   next to `merge_base`, wrapping `git merge-base --is-ancestor` with the
   same "non-zero exit reads as a plain `false`" convention
   `GitCli::commit_exists` already establishes for this file's call sites).
4. Local is an ancestor of remote and not equal ⇒ FR-3: `git update-ref
   refs/heads/<branch> <remote_tip>` and an `info!` log.
5. Local is *not* an ancestor of remote ⇒ FR-5: return
   `WorkspaceError::SharedStore { repo_name, branch, detail }` (existing
   variant, already carries free-text detail through the API to the caller)
   naming both commit ids, and skip the `update-ref` — the local branch is
   untouched.

FR-6 requires no code change: `store_resolves` already excludes a
remote-tracking ref from the cheap path (that's PR #175's fix), so
`origin/main`-shaped branches never reach the new method; they continue
through the existing full `publish_and_fetch` re-mirror on every call.

## Data Model

Not applicable — no new or changed persisted entity. (`WorkspaceError` is an
existing in-process error enum, not a stored model; no new variant is added,
the existing `SharedStore` variant is reused.)

## Contracts

Not applicable — no new or changed external API. The only caller-visible
change is that `SharedRepositoryStore::ensure(repo, branch)` can now return
`Err(WorkspaceError::SharedStore { .. })` for a *local*-shaped `branch` in a
situation (divergence) where it previously always returned `Ok`; that
`Err` already propagates through `create_cluster_workspace`
(`crates/local-deployment/src/container.rs`) to the workspace's
`creation_error` exactly like the existing `origin/<name>`-not-found case
does today (`ensure_reports_which_repository_and_branch_it_could_not_serve`).
No route or type signature changes.

## Research Notes

- Considered fetching from the *registered checkout* (`repo.path`) instead
  of the store's own `origin` remote, to match `publish_and_fetch`'s
  "mirror from the checkout" philosophy for the `refs/remotes/*` namespace.
  Rejected: a background-agent investigation (see conversation record)
  confirmed `repo.path`'s own local heads are never refreshed from the real
  GitHub remote by anything in this codebase, so mirroring from it would
  inherit the exact same staleness this feature exists to remove. Fetching
  the store's own `origin` (which already points at the real forge, per
  `configure()`) is the only option that is self-sufficient.
- Considered a 3-arg compare-and-swap `update-ref` (old value, new value) to
  guard against a concurrent `ensure` call for the same branch. Rejected:
  two concurrent callers converge on the same `remote_tip` from the same
  real origin at nearly the same time; a CAS would make the loser of a race
  fail outright for no benefit, where a plain 2-arg `update-ref` is
  idempotent and safe because the target branch name is never itself a
  workspace's checked-out worktree branch (those are always `vk/...`).
- Considered scoping the fix to also cover the non-clustered `repo.path`
  path. Rejected for this feature (see spec "Out of Scope"): `repo.path` is
  an ordinary working checkout, and force-moving a ref that may be the
  checkout's own `HEAD` risks desynchronizing the index/working tree from
  the ref — a materially different and riskier operation than updating a
  ref in a bare store with no working tree at all.
- No new dependency.

## Constitution Check

Governed by Principle LIX ("Vk's own git operations are hook-isolated and
remote-aware"), extended in this task (constitution v0.56.1) with the
paragraph requiring a shared-store local branch to be checked against the
remote and fast-forwarded when behind, and requiring workspace creation to
fail (not silently branch) when it has diverged. Also honors Principle II
(test the contract — two new `#[tokio::test]`s cover the fast-forward and
divergence paths) and Principle VI (don't rebuild what shipped — the fix
reuses `GitCli::fetch_with_refspec`, `resolve_commit`, the existing
`WorkspaceError::SharedStore` variant, and the file's own `commit_exists`
convention for a new `is_ancestor` primitive rather than inventing parallel
machinery). No deviation; no open question.

## Risks & Dependencies

- Every forge-reaching fetch this file performs already uses
  `GitHubCredentials::default()`; a private repository behind a repo-specific
  org token could still fail this freshness check silently (treated as
  FR-2's "unreachable" case) and fall back to the stale local head. This is
  a pre-existing limitation of the file, not a regression introduced here,
  and is called out in the technical spec's "Non-fixes" section as a
  separable follow-up.
- Depends on `git merge-base --is-ancestor` being available in the `git`
  binary the host resolves (it is — a long-standing, non-experimental git
  subcommand), and on the store's `origin` remote URL being fetchable with
  no credentials for public repositories, matching this file's existing
  fallback-fetch behavior.
