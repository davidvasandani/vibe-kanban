# Prior knowledge — VAS-1140 (source-repo push serialization)

Sources searched (read-only): `wiki/INDEX.md` and pages; `docs/knowledge-base/`.
No existing page covers `mirror_branch_back`, the `vk-registered` remote, or
stale worktree registrations in the source checkout. The task is new ground. The
pages below set its constraints.

## wiki/coordinator-nfs-load.md: single-flight pattern and NFS signature

- The incident signature matches it: high load, idle CPU, many
  `git -C /srv/vibe-kanban-shared/…` processes, and NFS waits that I/O PSI
  does not show.
- **Single-flight rule**: one slot per key. The **leader runs in its own
  `tokio::spawn` and owns the guard**. If the lock or permit lives in the
  request future, a cancelled request frees it while its git process keeps
  running, which breaks single-flight. The source mirror applies this as is:
  `request()` must not tie the slot to the caller's future.
- Bound the response, not only the work. A request handler must never wait on
  a slow git call. The mirror becomes fire-and-forget from `ensure_impl`.
- Rejected alternative recorded there: a periodic background refresher, which
  does work when nobody is looking. Our mirror stays request-triggered.

## wiki/worker-execution-hardening.md: killing process groups

- `kill_on_drop` kills only the direct child, not the group. Use
  `process_group(0)` and `killpg`.
- Save the leader pid at spawn, because `child.id()` is `None` after reaping.
  `killpg` the saved pgid even after the leader exits, to catch stragglers
  such as `git pack-objects`.
- `utils::process::kill_process_group_by_pgid` (SIGTERM, 2 s, SIGKILL)
  already exists.

## docs/knowledge-base/workspace-directory-reclamation.md: fail-safe deletion

- Retain on dirty **and** on indeterminate. `Path::exists()` conflates
  "absent" with "stat failed", so use `try_exists()`. Do not use
  `filter_map(|e| e.ok())` over `read_dir`, which drops unreadable entries.
- This governs the stale-registration scanner: any read or stat error means
  keep the entry.

## docs/knowledge-base/clustered-workspace-execution.md: shared mount

- "Treat a shared mount as a capability, not a directory": a missing path can
  mean a missing mount. This is why the stale-registration rule also requires
  the worktree's grandparent to exist, which proves the mount, before it calls
  a missing worktree deleted. `git worktree prune` has no such guard.

## wiki/capacity-backoff.md / refused-execution-recovery.md: backoff

- Exponential backoff with a cap, and errors surfaced with their own message,
  not a generic one.

## wiki/host-env-sensitive-tests.md: testing caveats

- The agent host sets `GIT_CONFIG_*`, which can leak into git-spawning tests.
  Make new tests hermetic: run their own `git` against temp dirs and do not
  depend on the operator's global config.
- Cold `cargo test` can exceed 10 min. Scope runs to `-p workspace-manager`
  and use `timeout`.

## Code facts gathered (not in the KB)

- `GitCli::git_impl_full` is synchronous `wait_with_output` with no timeout.
  It is used for every vk git call.
- `heal_cluster_worktree` runs on **every** `ensure_container_exists`, which
  has about 29 callers. Each call does `ensure_for_heal`, then
  `mirror_branch_back`, then `git push`.
- `adopt` rewrites only the worktree `.git` and the store admin dir. The old
  admin dir in `/srv/src/<repo>/.git/worktrees/` is never removed and keeps
  holding the branch.
