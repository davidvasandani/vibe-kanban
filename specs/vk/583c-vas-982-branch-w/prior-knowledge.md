# Prior knowledge: VAS-982 (shared clone branches from a stale local base ref)

Searched `docs/knowledge-base/INDEX.md`. The knowledge base is not empty; three
existing pages are directly on point and together already contain the root
cause and the design constraints for the fix. No other page in the index is
relevant to this task.

## `clustered-workspace-execution.md` — the load-bearing page

Section "The store must serve every name the picker can offer" and "A copy of
something that moves is not evidence of freshness" document the exact
mechanism this task fixes:

- The store resolves a target branch local-then-remote, matching
  `GitService::find_branch`'s order.
- **"A cheap-path guard that returns early because 'the store already has the
  branch' must mean a local head... A remote-tracking ref mirrors a branch
  that advances upstream, so accepting one there freezes the store at
  whatever commit it first learned: every later workspace branches from a
  stale base while the picker shows the current one, silently."**

This is the freshness rule PR #175 enforced for the *remote-tracking* shape
(`origin/main`) by excluding it from the cheap path entirely, forcing a
re-mirror every `ensure` call. It says nothing about the *local* shape
(`main`) ever needing the same treatment — because at the time, nothing
produced a bare local name as a real target branch in practice. This task's
incident is exactly that previously-unexercised case.

Also load-bearing for *how* to fix it safely:

- "`git fetch` is atomic across its refspecs" — one refused refspec (e.g. a
  wildcard that happens to touch a branch checked out by another workspace)
  discards every other refspec in the same invocation. Implication for this
  task: fetch the one target branch by itself, not folded into the existing
  wildcard mirror refspecs, and land it in the remote-tracking namespace
  (never directly into `refs/heads/<branch>`) so a concurrent read of the old
  local head is never disrupted mid-fetch.
- "A recovery fetch that cannot succeed is worse than none" — a refspec sent
  to the wrong remote, or in the wrong direction, either fails outright or
  (worse) lands silently under the wrong ref, where "local-first resolution
  then prefers it forever, at the wrong commit, on every node." Implication:
  fetch specifically `+refs/heads/<branch>:refs/remotes/origin/<branch>` from
  the store's own `origin` (the real forge, per `configure()`), never a
  local-to-local refspec against another path.

## `remote-only-branch-fetch.md` — the adjacent, already-fixed VAS-981 gap

Documents the *non-clustered* checkout's analogous miss: a branch absent
locally was never fetched before failing. Its "Resolve real credentials"
section is why this task's fix does **not** try to thread per-repo GitHub
credentials through the new fetch — that section's own precedent
(`publish_and_fetch`'s remote fallback fetch in `shared_repository.rs`,
same file this task touches) already uses `GitHubCredentials::default()`
for a forge-reaching fetch from the shared store, so this task's new fetch
follows established practice in the file it lives in rather than diverging
from it.

Its hooks-isolation and async-creation sections are not relevant here — they
concern a different failure mode (broken repo hooks; slow worktree checkout)
that this task does not touch.

## `workspace-branch-defaulting.md` — confirms bare local names are a real, reachable path

Branch precedence: explicit workflow branch > configured `default_target_branch`
> `origin/main` > `origin/master` > current checkout branch > first available.
A repository's admin-configured `default_target_branch` is a free-form string
that is not guaranteed to carry the `origin/` prefix — this is the production
path that puts a bare `main` into a workspace's `target_branch`, which is
exactly the shape the cheap-path freshness gap affects. (The create screen's
own default, when no override is configured, is already `origin/main`, which
this task's incident evidence — and the picker precedence above — makes
clear is *not* what sg-monorepo's workspace used.)

## Conclusion for this task

The fix is additive to the existing `SharedRepositoryStore::ensure` cheap
path: compare the local head against a freshly-fetched remote-tracking copy
of the same branch, fast-forward when behind, refuse when diverged. No
existing documented design decision is being reversed — the file's own
comments already named this as a known, deliberately deferred gap.
