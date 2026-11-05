# Feature Specification: Shared cluster store keeps a local base branch fresh before serving it

**Feature dir**: `specs/vk/583c-vas-982-branch-w/`
**Status**: Draft

## Summary

A clustered workspace's shared repository store resolves a requested base
branch once and then treats that resolution as permanent. When the base is
named in its bare local form (e.g. `main`, rather than the branch picker's
default `origin/main`), every workspace created after the first is silently
branched from whatever commit the store captured at first provisioning, no
matter how far the real branch has advanced since. This shipped a workspace
against sg-monorepo 1,680 commits behind its requested base, missing a
dependency and risking a merge that would have reverted months of work. This
feature makes the store check a resolving local base branch against the real
remote before serving it, fast-forwarding it when it is behind and refusing
workspace creation outright when it has diverged, so a workspace's base is
never silently stale or silently discarded.

## User Stories

- As an engineer starting a new cluster workspace against a repository whose
  default (or explicitly chosen) base branch is a bare local name, I want the
  workspace to start from that branch's current tip, so that I am not
  building on code that is months out of date without any indication.
- As an operator investigating a workspace that failed to create, I want a
  base branch that has diverged from its remote (e.g. because something wrote
  directly to the shared store) to produce a clear, actionable error instead
  of an inexplicably stale or silently altered workspace.

## Functional Requirements

- FR-1: Before a clustered workspace is provisioned, if its requested base
  branch already resolves as a **local** branch in that repository's shared
  store, the store MUST compare that branch's current tip against the same
  branch on the repository's real remote.
- FR-2: If the remote tip is unreachable (network failure, auth failure, no
  remote configured), the store MUST proceed with the existing local tip —
  the freshness check is best-effort and must never turn a store that could
  already serve the branch into one that cannot.
- FR-3: If the local tip is strictly behind the remote tip (every commit
  reachable from the local tip is also reachable from the remote tip, and
  they are not equal), the store MUST advance the local branch to the remote
  tip before the workspace is provisioned from it.
- FR-4: If the local tip and the remote tip are equal, no change is made.
- FR-5: If the local tip is not reachable from the remote tip (it holds at
  least one commit the remote does not — whether truly diverged or strictly
  ahead), workspace provisioning MUST fail with an error that names the
  repository, the branch, and both commit ids, and MUST NOT modify the local
  branch.
- FR-6: A base branch requested in its remote-tracking form (e.g.
  `origin/main`) is unaffected — that shape is already re-checked against the
  remote on every provisioning.

## Out of Scope

- The non-clustered (single registered-checkout) workspace-creation path.
  That checkout is an ordinary working directory, not a bare store; moving a
  branch ref that happens to be checked out there risks desynchronizing the
  working tree from the index, which is a different hazard needing its own
  design. Tracked as potential follow-up, not part of this feature.
- Periodic or scheduled refreshing of base branches independent of workspace
  creation.
- Changing which credentials are used for the remote comparison fetch, beyond
  what the shared store already uses for its existing forge-reaching fetches.

## Acceptance Criteria

- [ ] A shared store whose local base branch is strictly behind the branch's
      tip on the real remote produces a new workspace whose base is the
      remote's current tip, not the store's previously-captured commit.
- [ ] A shared store whose local base branch already matches the remote tip
      is unchanged by workspace creation (no spurious mutation, no error).
- [ ] A shared store whose local base branch has diverged from the remote
      (holds a commit the remote does not) causes workspace creation to fail
      with an error naming the repository, the branch, and both commit ids,
      and leaves the local branch ref exactly as it was.
- [ ] A base branch requested as `origin/<name>` continues to behave exactly
      as before this feature.
- [ ] An unreachable remote during the freshness check does not block
      creating a workspace the store could already serve.

## Open Questions

None. The incident, the exact code path, and the desired behavior were all
confirmed by reading the shared store's implementation and its existing
tests before this spec was written (see `technical-spec.md` and
`prior-knowledge.md` in this directory).
