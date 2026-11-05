# Technical Spec: new workspaces branch from the shared clone's stale local base ref

Task: VAS-982. Branch: `vk/583c-vas-982-branch-w`.

## Root cause (confirmed by reading the code, not just the incident report)

`SharedRepositoryStore::ensure` (`crates/workspace-manager/src/shared_repository.rs:80`)
is the cluster path's entry point for making a repository's shared bare store
resolve a workspace's requested base branch. Its cheap path:

```rust
if Self::store_resolves(&store, target_branch)? {
    Self::ensure_origin_fetch_refspec(&GitCli::new(), &store)?;
    let _ = self.mirror_branch_back(repo, target_branch);
    return Ok(store);
}
```

`store_resolves` (line ~612, pre-fix) only accepts a **local head**
(`refs/heads/<branch>`) as "already good" — and the file's own, pre-existing
doc comment on that function said so explicitly: *"A local head still
short-circuits, exactly as before this change. That carries its own staleness
for a local target branch, which is pre-existing behaviour and deliberately
left alone here."* That comment was written for PR #175
(`d71011fe`, "Serve remote-tracking target branches from the shared
repository store"), which fixed freshness for the *default* target branch
shape the create screen offers (`origin/main`) but explicitly punted on the
shape a repository with a configured `default_target_branch` of `main`, or a
user picking the local entry from the branch picker, produces: a bare local
name.

Once a bare local name (`main`) resolves once, it resolves forever: `ensure`
is idempotent by design ("cheap when the store is already good, because it
runs on every provisioning of every cluster workspace") and the cheap path
takes no action beyond a best-effort push *back* to the registered checkout
(`mirror_branch_back`) — it never pulls from anywhere. The store's local
`main` is therefore frozen at whatever commit the *first* workspace's
provisioning captured, for the lifetime of that repository's shared store.
Confirmed in the codebase, not just inferred from the incident: neither
`repo.path` (the registered checkout) nor the shared store has any
background job or periodic fetch that would otherwise correct this — the
only place `repo.path`'s remote-tracking refs get refreshed from the real
GitHub remote is the on-demand branch-status endpoint
(`crates/git/src/lib.rs:735` `get_remote_branch_status`, called from
`crates/server/src/routes/workspaces/git.rs`), which only updates
`refs/remotes/<remote>/*`, never `repo.path`'s own local heads, and isn't
called before workspace creation at all.

This matches the incident evidence precisely: sg-monorepo's workspace
requested base `main` (the bare form, not `origin/main`), and got branched
from the shared store's long-frozen local `main` — 1,680 commits behind.

## Why this is scoped to the shared (clustered) store, not the registered checkout

The non-clustered path (`WorkspaceManager::add_repository` /
`GitService::create_branch`, both operating directly on `repo.path`) has an
analogous gap — a local branch that already exists is never freshness
checked before a new branch is created from it. It is **not** fixed here,
deliberately:

- `repo.path` is an ordinary (non-bare) working checkout. Forcing its local
  `main` ref to a new commit when `main` happens to be the checked-out HEAD
  there would move the ref without touching the index or working tree,
  making every file between the old and new tip look locally modified or
  deleted to the next `git status` — a different and worse failure mode than
  the one being fixed.
- The shared store is always bare — there is no working tree to desynchronize,
  so a direct ref update is safe.
- The incident evidence is specifically about the shared bare clone ("shared
  clone's stale local `refs/heads/<base>`"); the registered-checkout case is
  a separate, lower-severity gap (it only affects single-node/non-clustered
  deployments) worth a follow-up task, not a scope increase on this one.

## Fix

Add `SharedRepositoryStore::fast_forward_local_head`, called from the cheap
path immediately after `store_resolves` returns `true`:

1. Read the store's own `origin` remote URL (copied from the registered
   checkout's remotes by `configure()` at store-creation time, and already
   pointed at the real forge, never at the checkout — see
   `fn configure`). If the store has no `origin` remote (a local-only
   registered repository), there is nothing upstream to be stale relative
   to: no-op.
2. Fetch `+refs/heads/<branch>:refs/remotes/origin/<branch>` from that URL —
   landing in the remote-tracking namespace, **not** overwriting
   `refs/heads/<branch>` directly, so the local head is never clobbered
   before its relationship to the fetched tip is known. If the fetch fails
   (unreachable forge, expired credentials), log a warning and continue
   serving the existing local head as-is — a store that can already serve
   the branch must not be blocked by a freshness check that can only move
   it forward.
3. Compare the fetched remote tip to the current local head:
   - Equal: no-op (already fresh).
   - Local is a strict ancestor of the remote tip (`GitCli::is_ancestor`,
     new helper, wraps `merge-base --is-ancestor`): fast-forward
     `refs/heads/<branch>` to the remote tip with `git update-ref` and log at
     `info!`. This is the ticket's "fetch `origin X`... or fast-forward
     local `X` first when it is strictly behind" requirement.
   - Neither is an ancestor of the other (true divergence), **or** the local
     head is strictly ahead (holds commits `origin` does not — e.g. a
     vk-side merge that moved the shared clone's ref independently, per the
     incident notes): refuse. Return `WorkspaceError::SharedStore` with a
     message naming both commits and explaining why ("a new workspace
     cannot be branched from it without either stranding the workspace on
     dead history or silently reverting work already on origin"). This
     satisfies "if local `X` has diverged ... fail with a clear error rather
     than silently using either side."

`WorkspaceError::SharedStore { repo_name, branch, detail }` is reused rather
than adding a new variant — it already exists specifically to carry a
free-text diagnosis of a shared-store branch problem back through the API to
the caller (see its doc comment: "a generic internal error turns a one-line
diagnosis into an investigation").

No change to the remote-tracking (`origin/main`) path: it is already
re-mirrored on every `ensure` call (that's what forced it off the cheap path
in #175), so it was never stale.

## Findings from independent review (Codex), and the fixes they produced

An independent Codex review of the first version of this fix (before this
section was added) found two real issues, both now fixed:

1. **`ensure` is also called with a workspace's own branch, not just a base
   branch.** `heal_cluster_worktree` (`crates/local-deployment/src/container.rs`)
   calls it with `workspace.branch` to materialise a `vk/…` branch ahead of
   `adopt` re-linking a pre-store worktree onto it. That worktree's on-disk
   files still reflect whatever commit they were created from; fast-forwarding
   (or divergence-refusing) that branch before adoption would hand `adopt` a
   target that no longer matches those files, and its `git reset -q` would
   then make every changed file look locally modified or deleted. Fixed by
   splitting `ensure` into `ensure` (checks freshness, for a base branch) and
   a new `ensure_for_heal` (never checks freshness, for a workspace's own
   branch), and moving `heal_cluster_worktree`'s call to the latter. A
   `branch_is_checked_out` guard at the top of `fast_forward_local_head`
   (skip entirely if any worktree in the store already has the branch
   checked out) is kept as defense-in-depth for the case this split doesn't
   cover — a branch already adopted and live.
2. **The cheap path's `update-ref` had no lock and no compare-and-swap**,
   so a concurrent `ensure` call for the same branch could overwrite a
   peer's newer write with an older one. Fixed with a 3-arg CAS
   `update-ref <ref> <new> <old>`; on contention, one bounded retry checks
   whether the peer's write is already at least as fresh (via `is_ancestor`)
   before attempting a second CAS, rather than either erroring or silently
   leaving a staler-than-necessary value in place indefinitely.

A third, lower-severity finding (`GitCli::is_ancestor` folding every
non-zero exit into `false`, not just exit 1) was left as-is: it matches the
pre-existing `commit_exists` convention in the same file, and both commits
passed to it are freshly confirmed present immediately beforehand, which
bounds the residual risk to the same level that convention already accepts
elsewhere in this file.

Two further review passes, prompted by the CAS retry introduced above,
found and fixed two more gaps before converging:

3. **The CAS retry's own divergence check couldn't tell "ahead" from
   "diverged" either** — it asked `!is_ancestor(remote_tip, current)`, which
   is equally true of a winner that is strictly ahead of what we fetched
   (fine) and one that has genuinely diverged (not fine), and would retry
   the CAS over a genuinely diverged commit.
4. **That same narrow check existed in the *initial* comparison too**, and
   was reachable by an ordinary (non-CAS-retry) race: caller A fetches an
   older remote tip, then a concurrent caller advances the local branch
   further before A reads it — A would misreport its own, already-fresher
   local branch as diverged and fail workspace creation that should have
   succeeded.

Both are fixed by one shared four-way classifier,
`SharedRepositoryStore::classify_against_remote` (equal / behind / ahead /
diverged, via up to two `is_ancestor` calls), used identically by the
initial check and the CAS retry's re-read. Only `Diverged` errors; `Ahead`
and `Equal` are both no-ops; `Behind` fast-forwards (or, on retry,
retries). A unit test,
`classify_against_remote_distinguishes_ahead_from_diverged`, pins the exact
case that was wrong.

## New `GitCli` primitive

`GitCli::is_ancestor(repo_path, ancestor, descendant) -> Result<bool,
GitCliError>` wraps `git merge-base --is-ancestor`, following the exact
pre-existing pattern of `commit_exists`: exit 1 ("no") is reported as `Ok(false)`,
not an error; only a genuinely failed command (not this specific expected
negative) propagates. Added next to `merge_base` in `crates/git/src/cli.rs`.

## Acceptance criteria mapping

| Ticket requirement | Where it's met |
|---|---|
| Fetch `origin X` and branch from the remote-tracking ref, or fast-forward local `X` when strictly behind | `fast_forward_local_head`, step 3, ancestor case |
| Never branch from a local ref behind origin without a warning | `info!` log on fast-forward; the ref is moved before `ensure` returns, so no caller ever branches from the stale commit |
| Diverged local base fails with a clear error | `fast_forward_local_head`, step 3, non-ancestor case → `WorkspaceError::SharedStore` |
| Regression test: a shared clone whose local `main` is N commits behind origin produces a workspace at origin's tip | `ensure_fast_forwards_a_stale_local_target_branch_to_origin` |

## Non-fixes / explicitly out of scope

- Periodic/background fast-forwarding of local base refs (the ticket's
  "Optional" bullet). The fix above is on-demand (every `ensure` call, i.e.
  every workspace provisioning), which already prevents staleness from
  compounding across workspaces; a periodic job would only matter for a repo
  with no new workspaces for a long time, which is a much smaller problem
  than the one reported.
- The non-clustered `repo.path` path (see "Why this is scoped" above).
- Threading real per-repo GitHub credentials through this fetch. The
  existing code in this same file already performs forge-reaching fetches
  with `GitHubCredentials::default()` in the equivalent fallback path inside
  `publish_and_fetch` (`cli.fetch_with_refspec(store, &url, &refspec)`,
  no credentials parameter) — this change follows the same, already-accepted
  pattern rather than introducing a different one for just this call site.
  Wiring real credentials through `ensure`'s whole call chain is a larger,
  separable change.
