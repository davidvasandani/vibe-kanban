# Source-checkout branch mirror and worktree registrations

For a clustered workspace the branch is authoritative in the **shared store**
(`<shared>/repositories/<repo-id>`). vk keeps a *courtesy copy* in the
operator's registered checkout (`/srv/src/<repo>`), so tools and older
releases that look there still find it.
`crates/workspace-manager/src/source_mirror.rs` owns that copy.

## The incident (VAS-1140, 2026-11-28)

`ensure_container_exists` runs on almost every workspace request. It heals the
worktree, then calls `mirror_branch_back`, which was a **synchronous, untimed
`git push` inside the request**.

- A 1.6 GB agent-committed branch led to 44 concurrent pushes, the oldest
  running for 77 minutes.
- Load on think2 reached 127, the DB pool timed out, and follow-ups hung at
  "Sending".
- Every push was doomed anyway. A leftover registration in
  `/srv/src/homelab/.git/worktrees/` still recorded the branch as checked out.
  `adopt` re-linked worktrees into the store but never retired the old entry.

## Rules the mirror follows (constitution LXI)

- **A request never waits on it.** `SourceMirror::request` is O(1). It runs at
  most one leader task per (registered path, branch). A trigger that arrives
  during a flight sets a follow-up flag, so a burst becomes at most one more
  round. The leader owns the slot, the same pattern as
  [[coordinator-nfs-load]].
- **The steady state spawns nothing.** Both refs are compared from the
  filesystem (loose ref or `packed-refs`). Only on a mismatch, or when the
  filesystem cannot tell (reftable, a symbolic ref), does it fall back to
  `rev-parse`.
- **Everything is bounded.** Every git child runs in its own process group.
  The pipes are drained *before* the leader is reaped, so the unreaped leader
  pins the pgid and the whole group (`pack-objects`, `receive-pack`) can be
  `killpg`ed safely at the deadline. **Never signal a pgid after its leader is
  reaped**, because the number can be reused. After reaping, only probe group
  members through `/proc` and ignore zombies
  (`utils::process::process_group_has_running_members`).
- **Fence timed-out work instead of forgetting it.** A killed child stuck in
  D-state on a hard NFS mount, or a blocked `spawn_blocking` scan, becomes a
  *leftover*. No new round starts while one is alive, and the attempt is
  reported as a timeout. A per-repository async lock serializes pushes across
  aliased registrations. It is acquired with a bound, and a timed-out push
  keeps it until the push exits.
- **Failures are visible.**
  - Each failure is logged at `warn!` with a bounded tail of git's stderr.
  - It is shown on the workspace through `RepoBranchStatus.source_mirror_error`,
    rendered in `RepoCard`.
  - It is classified as deterministic, transient or timeout. A ref-lock
    `remote rejected (failed to update ref)` counts as transient.
  - Retries back off exponentially: base 1 min, capped at 30 min, or 6 h for
    deterministic failures. A deterministic push refusal is retried only when
    the store or destination tip changes. A "checked out" refusal is never
    gated, because pre-flight re-checks it every round.
- Knobs: `VK_SOURCE_MIRROR_PUSH_TIMEOUT_SECS` (default 300) and
  `VK_SOURCE_MIRROR_DISABLED=1`.

## Worktree registrations: what counts as evidence

`utils::worktree_linkage::registration_state` reads only the filesystem.

- **Re-linked → `Stale`.** The worktree's `.git` names *another* existing
  registration whose `gitdir` points back at this worktree. Normalise relative
  `gitdir` paths lexically first (`worktree.useRelativePaths`). Only this state
  can be retired without further evidence.
- **Missing → `WorktreeMissing`, never `Stale`.** A missing directory looks
  exactly like an offline mount, and an unmounted mount point can even expose
  a non-empty underlying directory. Use `symlink_metadata`: a dangling symlink
  into an offline volume has not been deleted. Removal needs the caller's own
  deletion evidence. Workspace delete saw the worktree, recorded its `st_dev`,
  deleted it, and re-checks at the removal boundary that the base directory is
  still on that device.
- **Never `git worktree prune`.** It decides on directory presence across *all*
  entries and has no mount guard. It was deliberately removed from cleanup
  before this task (`worktree_manager.rs`).
- **Retire; never delete.** Snapshot the entry's `index` to
  `refs/vk-retired/<name>-<epoch>`; the index can be the only record of
  staged-only content. Pin a detached HEAD the same way. Then *move* the
  directory to `<common>/vk-retired-worktrees/`. If any of these steps cannot
  be done, keep the entry. Re-read the entry under a per-entry lock right
  before acting, because git may have reused the admin path.
- **The startup sweep retires re-linked entries only.** Missing ones are kept
  and named by mirror pre-flight. The existing pile in `/srv/src/homelab` needs
  a one-time manual, per-entry clean-up.

## Things that read the courtesy copy must not trust it

With the copy asynchronous, every reader of `repo.path`'s workspace branch was
audited.

- **Branch status** reads the workspace side from the worktree's own
  repository. A local target is compared as the checkout has it *now*: the
  missing target commit is fetched into the store in the background,
  single-flight with back-off, and the counts are `null` until it lands.
  Unknown counts must not render as "Up to date".
- **Direct merge** pins the store's branch tip, then:
  1. fetches that commit's objects into the checkout (`FETCH_HEAD` only);
  2. merges exactly that commit (`GitService::merge_changes_pinned`);
  3. records the merge;
  4. moves the store branch by compare-and-swap;
  5. only then moves the checkout copy, if it was a lagging ancestor and is not
     checked out (`advance_lagging_copy`).

  Moving the copy first would put it on history the authoritative branch never
  had, and every later non-forced push would be rejected.
- **Existing bug fixed along the way:** `get_remote_branch_status` resolved
  the base ref *before* fetching, so it compared against the stale tip. It now
  re-resolves after the fetch.

## Open follow-ups

- `get_remote_branch_status`'s network fetch has no process-level deadline.
  Callers are off-worker, bounded and single-flight per repository, but a hung
  fetch still holds a blocking thread.
- The store's copied `origin` URL does not follow changes to the checkout's.

## Contributed by

- `vk/2b38-vas-1140-seriali`
