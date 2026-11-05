# Fetching a branch that only exists on the remote

## A fetched ref must land where the re-check (and `git worktree add`) will look

`GitService::check_branch_exists` resolves a bare branch name through git2's
`repo.find_branch(name, BranchType::Local)` then `find_branch(name,
BranchType::Remote)`. The `Remote` lookup is not "does any remote know this
branch" — git2 resolves it as a literal ref name under `refs/remotes/`, so it
only matches when the string you pass already includes the remote, e.g.
`"origin/feat/x"`. Passing the bare simple name (`"feat/x"`) and fetching it
into `refs/remotes/origin/feat/x` leaves both lookups failing: `Local` doesn't
see it, and `Remote` is looking for `refs/remotes/feat/x` (no `origin/`
prefix), which doesn't exist either. The fetch silently "succeeds" and the
next branch-existence check still reports the branch missing.

For a workspace targeting its own registered (non-clustered) checkout, land
the fetch directly as a local branch instead: `+refs/heads/<branch>:refs/heads/<branch>`.
That's exactly what a bare-name re-check resolves under `BranchType::Local`,
and exactly what `git worktree add <path> <branch>` resolves without any DWIM
remote-tracking fallback. This is a different case from the shared cluster
store described in
[clustered-workspace-execution](clustered-workspace-execution.md) (`git fetch`
being atomic per refspec, and why a remote-tracking ref in the *store* is never
evidence of freshness) — this page is about the single-checkout, non-clustered
path, where "land it where the checkout's own revision-resolution will find
it" is the whole fix.

## Resolve real credentials; don't default past a repo's configured org token

A new server-initiated git network call (fetch, push, `ls-remote`) on an
already-registered checkout must resolve the repo's actual GitHub credentials
through `services::github_credentials::resolve_for_repo(pool, git,
&repo.path)` and pass the result down explicitly — the same pattern already
used in `routes/workspaces/pr.rs`. `GitHubCredentials::default()` is not "no
credentials," it's "every owner uses the ambient fallback," which silently
bypasses a repo's configured org token from Settings → Repositories → GitHub
organization tokens. A lower-level crate (e.g. `workspace-manager`) that needs
this should take `&GitHubCredentials` as a parameter rather than resolve it
itself — `resolve_for_repo` lives in `services`, and crates below `server`
should not take on that dependency; resolve once at the `server`-crate call
site and thread it through.

## A repository's own hooks must never be able to break vk's git plumbing

Every `GitCli` command (worktree add/remove/move/repair, checkout, merge,
rebase, commit, fetch, push) funnels through one function
(`GitCli::git_impl_full`). It now passes `-c core.hooksPath=/dev/null`
unconditionally. Before this, a workspace running a repository's own
`postinstall` (e.g. `lefthook install`) could write hook shims into the
*shared, cross-workspace* checkout's common git directory — not its own
worktree — and a broken hook (a bad `.lefthookrc` lookup, in the incident that
surfaced this) then failed `git worktree add` for every other workspace on
that repository until someone noticed and removed the hooks by hand. Centralizing
the disable at the one low-level command-execution point covers every call
site, present and future, rather than requiring each one to remember it (the
pre-existing precedent, `crates/remote/src/github_app/service.rs`, only
disabled hooks for its own checkout calls).

## Workspace creation is intentionally asynchronous — a quiet `list_sessions` isn't proof of failure

`start_workspace`/`create_and_start_workspace` persists the workspace row and
returns immediately; the worktree and first session are created by a detached
background task. A large repository's worktree checkout over NFS can
legitimately take about a minute. Before concluding a workspace is stuck,
check `creation_status` (`Queued` / `Running` / `Ready` / `Failed`) and
`creation_error` on the workspace record — `create_session` now surfaces
these instead of a generic "Workspace not found," and a `Failed` status
carries the real underlying error (e.g. a missing branch or a worktree
failure), not a placeholder.
