# Feature Specification: Portable Git worktrees for cluster-placed workspaces

**Feature dir**: `specs/vk/19a4-git-worktrees-br/`
**Status**: Draft

## Summary

Every workspace that Vibe Kanban places on a worker node currently has an
unusable Git worktree. The coordinator creates the worktree from a
coordinator-local repository path, and Git records absolute paths in both
directions, so the worktree's `gitdir:` pointer names a location that does not
exist on the worker — or, worse, names a same-named local directory holding a
different repository. Every Git command an agent runs in its workspace fails with
`fatal: not a git repository: (null)`, and *every* cluster-placed workspace is
affected, across all projects and organisations. Measured on the live share:
15 worktrees across 9 cluster workspace directories, 15 broken, 0 resolving —
100% of cluster-placed workspaces, not a partial failure. (The "~130 workspaces"
figure quoted in PR #172's description counts all workspaces on `think2`,
overwhelmingly `placement_state = local`, which are unaffected.)

This feature makes Vibe Kanban own a repository store on shared storage, so a
worktree's backing repository resolves identically on every node; enforces that
property instead of relying on an operator convention; and heals the existing
broken workspaces in place without destroying the agent work they hold.

## User Stories

- As an agent running on a worker, I want `git status`, `git diff`, `git log` and
  `git commit` to work in my workspace, so that I can do the task I was
  dispatched for.
- As a user with an existing broken workspace, I want it repaired without losing
  my edits, my untracked files, or commits an agent already made, so that I do
  not have to start the task over.
- As a user, I want a workspace that cannot be made usable to say so at
  provisioning time, so that I do not discover it several minutes into an agent
  turn.
- As a maintainer, I want the property that makes worktrees usable across nodes
  to be enforced by the system rather than documented, so that a misconfigured
  repository cannot silently produce broken workspaces again.
- As an operator, I want to register a repository anywhere on the coordinator and
  still get working cluster workspaces, so that repository location and cluster
  correctness are independent concerns.

## Functional Requirements

**Portability**

- **FR-1:** A workspace that may execute on a worker MUST have every one of its
  worktrees backed by a repository whose location resolves to the same object on
  the coordinator and on every worker.
- **FR-2:** The system MUST establish FR-1 itself. It MUST NOT depend on the
  operator having registered the repository in a particular location.
- **FR-3:** FR-1 MUST be asserted structurally — that the resolved backing
  repository lies within the shared storage root — and never by testing the path
  text against a known-bad prefix.
- **FR-4:** A directory that merely exists at the expected location MUST NOT
  satisfy FR-1. A same-named local repository is not the backing repository.
- **FR-5:** Both directions of the worktree link MUST be verified together: the
  worktree's pointer to its administration record, and that record's pointer back
  to the worktree.

**Provisioning**

- **FR-6:** Provisioning MUST create the shared backing repository for each of a
  workspace's repositories before creating worktrees, and this MUST be safe to
  repeat. It happens inside the existing provisioning state; no new state and no
  new asynchrony is introduced. The long-running copy MUST NOT hold the
  repository administration lease, which a multi-minute clone could outlive; the
  lease covers only the short verify-and-publish step.
- **FR-7:** A workspace whose worktrees cannot be made to satisfy FR-1 MUST fail
  provisioning with an operator-readable reason naming the repository. It MUST
  NOT be reported as ready.
- **FR-8:** Creating the shared backing repository MUST be atomic: a partially
  created one MUST never be observable as usable.
- **FR-9:** The shared backing repository MUST be able to reach the same remotes
  as the registered repository, so that pushing branches and opening pull
  requests continue to work.
- **FR-10:** The branch a workspace is based on MUST be proven present in the
  shared backing repository before the workspace is used. Its absence is a
  failure, not a reason to substitute a different branch.

**Existing broken workspaces**

- **FR-11:** An existing broken workspace MUST be repaired in place. Its
  directory MUST NOT be recreated, re-cloned, or replaced.
- **FR-12:** Repair MUST preserve tracked file contents, untracked files, and any
  commits already made on the workspace branch.
- **FR-13:** Repair MUST refuse, and report, rather than proceed when it cannot
  prove the workspace's branch and its commits are present in the shared backing
  repository.
- **FR-14:** Repair MUST refuse when the workspace's branch is already in use by
  a different workspace.
- **FR-15:** Repair MUST be idempotent, and MUST be a cheap no-op for a workspace
  that already satisfies FR-1.
- **FR-16:** Repair MUST be per-workspace best-effort with a truthful aggregate
  result: one repository failing MUST NOT abort the others, and MUST NOT let the
  workspace report success.
- **FR-17:** Repair MUST NOT act on a workspace whose assigned worker is
  unreachable. Unreachable means indeterminate, not idle. Reachability MUST be
  read from the lease and heartbeat records that already govern dispatch — the
  evidence channel — and never from a health endpoint or a metrics surface, which
  are read-only observability and not admissible as the basis for a lifecycle
  decision.
- **FR-18:** Repair MUST log the target, why it was selected, and the action, at
  a level visible in production, before acting.

**Enforcement over time**

- **FR-19:** FR-1 MUST be checked on startup, at placement, and before use — not
  only where a worktree is created. The startup check is a bounded, concurrency-
  limited sweep that repairs what it can and reports an aggregate; repair also
  happens lazily whenever a cluster workspace is touched. Neither requires an
  operator to trigger it.
- **FR-20:** A check MUST enumerate every violation in one pass with an
  actionable remedy, rather than stopping at the first.
- **FR-21:** A workspace that is not cluster-placed MUST be reported as *not
  applicable*, a distinct outcome from *broken*.
- **FR-22:** A worker asked to run work in a workspace whose worktree does not
  resolve MUST refuse the dispatch with a specific reason, and the resulting job
  record MUST reach a terminal state.
- **FR-23:** A worker MUST NOT attempt to repair a worktree. Repair authority
  stays with the coordinator.

**Blast radius**

- **FR-24:** Consolidating every workspace of a repository onto one shared
  backing repository MUST NOT let one workspace's cleanup affect another
  workspace's worktree or its administration record.
- **FR-25:** Automatic repository maintenance MUST NOT run against the shared
  backing repository, because it can remove other workspaces' administration
  records as a side effect.

**Compatibility**

- **FR-26:** Workspaces that run on the coordinator MUST be unaffected: same
  directory, same backing repository, same behaviour as before this change. The
  distinction MUST be drawn from the persisted placement state, which has six
  values (`local`, `reserved`, `provisioning`, `ready`, `failed`, `cleaning`).
  Only `local` uses the registered repository; the other five — **`cleaning`
  included** — use the shared backing repository, because a cluster workspace
  being cleaned up must have its worktrees administered in the repository they
  were created from. The mapping MUST be exhaustive over the placement states,
  with no catch-all case, so that adding a state is a build failure rather than a
  silent fallthrough to the registered repository.
- **FR-27:** Installations with clustering disabled MUST be unaffected. No shared
  backing repository is created and no behaviour changes.
- **FR-28:** Reverting to the previous release MUST NOT cause repaired workspaces
  to be destructively recreated. This is achieved by keeping the workspace branch
  present in the registered repository — best-effort, on every provisioning, not
  only at repair — so a previous release still resolves branch-scoped operations.
  An explicit forward-only gate is rejected: it would disable the deploy loop's
  automatic rollback, which is a larger safety mechanism than the degradation it
  would prevent.

**Lifetime**

- **FR-29:** A shared backing repository MUST be retained while any workspace
  references its repository. It MUST NOT be removed as a side effect of deleting
  a single workspace. It MAY be reclaimed only when the repository record is gone
  and no workspace references it, and an error while determining that MUST result
  in retention.

## Out of Scope

- Changes to the dispatch protocol, request signing, heartbeats, leases,
  scheduling, or preview routing.
- Multiple coordinators. Repository administration remains single-owner.
- Changes to deployment: the shared mount, its export, and the coordinator's
  local source checkouts stay as they are.
- Moving coordinator-local workspaces to shared storage.
- A maintenance, repacking, or quota policy for the shared backing repositories
  beyond disabling automatic maintenance. Their *lifetime* is specified (FR-29);
  their *upkeep* is not.

## Acceptance Criteria

- [ ] A newly created cluster workspace's worktree resolves within shared
      storage, and `git status`, `git log`, `git diff` and `git commit` all
      succeed when run on the assigned worker.
- [ ] An existing broken workspace, when next opened, becomes usable, and its
      tracked contents, untracked files, and any commits an agent already made
      are all still present.
- [ ] Repairing the same workspace twice changes nothing the second time.
- [ ] A workspace whose repository cannot be made portable is reported failed,
      with a reason naming the repository, and is never reported ready.
- [ ] Dispatching into a workspace whose worktree does not resolve is refused
      with a specific reason, and the job record ends in a terminal state.
- [ ] A worker never repairs a worktree and never substitutes a same-named local
      repository.
- [ ] Two workspaces of one repository can be created and one deleted, leaving
      the other's worktree and administration record intact.
- [ ] A coordinator-placed workspace behaves exactly as it did before this
      change.
- [ ] With clustering disabled, no shared backing repository is created and no
      behaviour changes.
- [ ] Branch status, rebase, merge, push and pull-request creation succeed for a
      cluster workspace.
- [ ] A two-node exercise — create a workspace on a worker, run an agent turn
      that commits, open a pull request, restart the coordinator, remove the
      shared mount — leaves worktrees intact and reports the mount loss rather
      than degrading silently.

## Open Questions

None. All four are resolved in [`clarifications.md`](clarifications.md):
provisioning blocks inside the existing state with the clone outside the lease
(C1); repair runs both as a bounded startup sweep and lazily, with no operator
trigger (C2); a backing repository is retained while referenced (C3); and branch
push-back is sufficient for rollback safety, with a forward-only gate rejected
(C4).

Two items are recorded as **accepted risks**, not open questions: the
post-rollback divergence window (C4), and NFSv3 lock semantics for concurrent ref
updates — measured clean on the production export, but the two-node deployment
gate remains the acceptance authority.
