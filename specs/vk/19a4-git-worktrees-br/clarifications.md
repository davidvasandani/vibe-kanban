# Clarifications — `19a4-git-worktrees-br`

All four open questions from `spec.md` are resolved. Each answer is grounded in
the observed deployment: one coordinator (`think2`) whose cluster placements are
broken without exception — 15 worktrees across 9 cluster workspaces, 15 broken,
0 resolving — workers `think3`/`think4`, an NFSv3 export with 35 TB of 37 TB
free, a deploy loop that health-probes and auto-rolls-back, and the existing
SQLite-fenced repository administration lease. (PR #172's "~130" counts all
workspaces on `think2`, overwhelmingly `placement_state = local` and unaffected.)

---

## C1 — Blocking vs asynchronous creation of the shared backing repository

**Question.** Should the first cluster workspace request for a repository block
while its shared backing repository is created (potentially minutes for a large
repository), or should provisioning return "preparing" and complete
asynchronously?

**Answer. Block, inside the placement state machine's existing `provisioning`
state. Add no new state and no new asynchrony.**

`create_cluster_workspace` already transitions `reserved → provisioning →
ready|failed`, and the user already sees a workspace as provisioning while its
worktrees are created. Store creation is one more step inside that window, so it
is *already* asynchronous from the user's point of view. Introducing a second
"preparing" concept would duplicate a state machine that exists and works.

**However, the clone must not run inside the fenced lease.** The repository
administration lease is a bounded SQLite lease, and a multi-minute clone of a
large repository could outlive it — which would silently unfence exactly the
operation the lease exists to protect. The work is therefore split:

1. Clone into the per-repository staging directory `repositories/.{repo_id}.incoming`
   **outside** the lease. Staging names are per-attempt, so concurrent attempts
   cannot corrupt each other.
2. Take the lease only for verify → configure → `rename(2)` into place → fetch.
   These are short and bounded.

Concurrent provisionings for the same repository are deduplicated **by the
administration lease itself**, not by any in-process mutex. Under the lease, the
first thing `ensure` does is re-run its early-out check; the loser of the race
therefore observes the winner's published store and discards its own staging
directory, so two workspaces created at once produce one store. There is no
cross-crate mutex to lean on here — `repository_operation_lock` and its
`REPOSITORY_OPERATION_LOCKS` static are private to `crates/worktree-manager`
(`worktree_manager.rs:189`) and unreachable from `crates/workspace-manager` — and
no new one is minted. The loser wastes a clone, which is the correct trade
against a second lock with its own ordering rules.

**Spec effect.** FR-6 gains: creation happens within the existing provisioning
state; the long-running clone runs outside the administration lease, which is
held only for the short verify-and-publish step.

---

## C2 — Automatic fleet-wide repair vs lazy repair

**Question.** Should repair of the existing broken workspaces — all 15 worktrees
across 9 cluster workspaces, i.e. the entire cluster-placed fleet — run
automatically on the first startup after upgrade, or only lazily as each
workspace is opened, with a fleet-wide pass triggered explicitly by an operator?

**Answer. Both, automatically. A bounded fleet-wide sweep on coordinator startup,
plus lazy repair whenever a cluster workspace is touched. No operator trigger.**

Three reasons:

- The fleet is broken *now*, across all projects and organisations. Requiring an
  operator action to fix it makes the outage last as long as it takes someone to
  read the release notes.
- Repair is non-destructive (pointer files only), idempotent, and a cheap no-op
  for an already-portable workspace, so a recurring sweep is affordable.
- A one-off migration with no recurring check is a comment, not a control
  (constitution XX). The sweep is the level-triggered enforcement of FR-19, not a
  migration script that runs once.

Bounds on the sweep, all required:

- Concurrency-limited. Today's 15 worktrees would not stampede the NFS export,
  but the bound is on the sweep, not on today's fleet size, and the fleet grows
  by one worktree per cluster workspace created.
- Per-workspace failure isolation with a truthful aggregate (FR-16).
- Skips any workspace whose assigned worker is unreachable (FR-17).
- Walks `{shared_root}/workspaces` only, so the deploy build worktree under
  `/srv/src/vibe-kanban-rebuild-cache/build-tree` is never in range.
- Emits one operator-visible summary — repaired / skipped / failed with reasons —
  through structured logging and the placement reason. Both are in-process and
  already surfaced. No new alert channel, and no external notifier:
  `vk-deploy-notify` is a systemd/Nix helper in
  `homelab/modules/vibe-kanban-rebuild.nix`, not a Rust API, and that module is
  out of scope.

**Spec effect.** FR-19 gains "and on coordinator startup as a bounded sweep";
acceptance gains an assertion that the startup sweep is concurrency-limited and
reports an aggregate.

---

## C3 — Lifetime of a shared backing repository when a repository is deleted

**Question.** Immediate removal, retention until unreferenced, or indefinite
retention?

**Answer. Retain while any workspace references the repository. Reclaim only
through the existing workspace-reclamation sweep, only when the repository record
is gone *and* no workspace references it, and never as a side effect of deleting
a single workspace.**

The store holds every cluster workspace's objects and refs for that repository.
Deleting it while a workspace still exists would destroy exactly the agent work
this feature was written to protect. The governing rule is retain-on-doubt: an
error while determining whether the store is referenced is not evidence that it
is unreferenced, and the sweep retains in that case.

35 TB free against one object store per repository (not per workspace) means
retention costs nothing measurable, so the fail-safe direction is unambiguous.
Reclamation, when it does happen, is logged with the path and the reason before
acting.

**Spec effect.** New FR-29 covering store lifetime; out-of-scope note updated so
"no maintenance policy" no longer reads as "no lifetime rule".

---

## C4 — Is branch push-back sufficient for rollback safety, or is a forward-only gate needed?

**Question.** Does FR-28 need an explicit gate blocking reverts to a release
without this change?

**Answer. Push-back is sufficient. No forward-only gate.**

Tracing the rollback case concretely: after a revert, the old binary calls
`ensure_workspace_exists` → `check_branch_exists(&repo.path, branch)` →
`ensure_worktree_exists` → `try_repair_worktree_in_place`, and only reaches
destructive recreation if that repair path cannot leave the worktree on the
expected branch. A repaired worktree is still fully functional **on the
coordinator**, because the coordinator also mounts the shared export — so the old
binary reads the expected branch, `try_repair_worktree_in_place` succeeds, and
recreation is never reached. The destructive path is not on the rollback route.

What the old binary *would* get wrong is branch-scoped operations resolved
against `repo.path` — base commit, diff, merge, pull request — because the branch
lives in the store. Pushing the workspace branch back into the registered
repository fixes precisely that, and it is a cheap local ref update.

A forward-only gate is actively harmful here: it would disable the deploy loop's
automatic rollback, which is the fleet's main safety mechanism against a bad
release. Trading a reliable safety net for a rollback-only degradation is the
wrong direction.

**Residual risk, accepted and documented.** After a rollback, commits an agent
makes land in the store, while the old binary's merge reads the registered
repository's tip. Mitigation: push-back is best-effort on every `ensure`, not
only at adoption, so the registered repository tracks the store's branch tips for
as long as the fixed binary is running. The window is bounded by how long a
rolled-back release stays deployed.

**Spec effect.** FR-28 gains the mechanism (push-back, best-effort, on every
ensure) and the explicit rejection of a forward-only gate.

---

## Remaining open questions

None. Two items are recorded as accepted risks rather than open questions:

- The post-rollback divergence window described in C4.
- NFSv3 lock semantics for concurrent ref updates. Measured clean on the
  production export (four worktrees × 15 concurrent commits, five correct branch
  heads, clean `fsck`), but measurement is not proof; the two-node deployment
  gate remains the acceptance authority.
