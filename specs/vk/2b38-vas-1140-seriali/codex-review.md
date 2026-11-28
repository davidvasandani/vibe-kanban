# Codex review: VAS-1140

Tool: `codex review --base origin/main` (Codex CLI 0.171.0), 38 rounds. Each
round's confirmed findings were fixed and re-verified (clippy `-D warnings`
and tests) before the next round.

## What the rounds changed, beyond the original plan

- **Fencing.** A git child or filesystem scan that times out is kept as a
  *leftover*. No new round starts while one is alive. The process group is
  killed while the leader is still unreaped, so its pgid is still pinned.
  After reaping, the group is only ever probed through `/proc`, and zombies
  do not count. Pipes are drained before the leader is reaped. Each pipe
  buffer keeps only a bounded tail.
- **Steady state spawns nothing.** Refs are compared from the filesystem
  (loose ref or `packed-refs`). The re-check under the per-repository push lock
  stops a duplicate push from an aliased registration.
- **Retry gate.** A deterministic refusal is gated on (store tip, destination
  tip). A "checked out" refusal is never gated, because pre-flight re-checks
  it on every round.
- **Registration cleanup is fail-safe.**
  - Re-linked entries are retired only when the replacement registration
    points back at this worktree. Relative paths are normalised first.
  - Before anything moves, the index is snapshotted to
    `refs/vk-retired/<name>-<epoch>` and a detached HEAD is pinned. The entry
    is then moved to `vk-retired-worktrees/`; it is never deleted.
  - Missing-worktree entries are removed only on vk's own deletion evidence:
    the worktree was seen before cleanup, and is gone afterwards on the same
    device.
  - The startup sweep keeps missing-worktree entries. See the spec's FR-011
    revision.
- **Direct merge no longer depends on the courtesy copy.**
  - It pins the store's branch tip, fetches that commit's objects into the
    checkout, and merges exactly that commit (`merge_changes_pinned`).
  - The store branch then follows the squash commit by compare-and-swap.
    After that, the checkout copy follows if it was a lagging ancestor and is
    not checked out (`advance_lagging_copy`).
  - The merge is recorded before the hand-off. The hand-off runs detached,
    and archiving waits for it with a bound.
- **Branch status reads the workspace side from the worktree's repository.**
  - Local targets are compared as the checkout has them now. A missing target
    commit is fetched in the background, single-flight with back-off; until it
    lands the counts are unknown.
  - Remote comparisons run off the runtime workers, bounded, one fetch per
    repository at a time.
  - Unknown counts are never shown as "Up to date".
  - `get_remote_branch_status` now re-resolves the base ref after fetching.
    Before, it compared against the pre-fetch tip; this was an existing bug.

## Not addressed, with reasons

- **Process-level deadline inside `get_remote_branch_status`'s libgit2/CLI
  fetch.** Existing behaviour; the status handler already ran this fetch
  inline on a runtime worker before this change. It is now off-worker, its
  wait is bounded to 20 s, and it is single-flight per repository. A hung
  fetch still holds a blocking thread. Fixing that means making
  `GitCli::git_network` deadline-aware for every network operation. Follow-up.
- **The store's copied `origin` URL drifting from the checkout's.** Existing
  store configuration on `origin/main`. It is out of scope here.
- **The existing-store fetch-refspec repair running outside the lease.** This
  code is already on `origin/main` and this diff does not change it.
- **Startup sweep removing missing-worktree entries.** Deliberately not done
  (constitution XV and LXI). No evidence available at startup distinguishes a
  deletion from an offline mount in every layout. Such entries are named by
  mirror pre-flight, with per-entry, index-preserving guidance for the
  operator.
- **Measuring the auto-commit's OID instead of HEAD.** An earlier round asked
  for the HEAD read to move off the completion path. The background task now
  reads HEAD first thing, under a deadline. `GitService::commit` returns no
  OID.
