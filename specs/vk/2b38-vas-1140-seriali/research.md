# Research — VAS-1140

## R1. Where the push comes from
- Decision: `SharedRepositoryStore::mirror_branch_back` is the only caller of a
  push into `repo.path`. `ensure_impl` reaches it through
  `heal_cluster_worktree`, which runs on every `ensure_container_exists`.
  That function has about 29 route callers.
- Alternative rejected: changing how often heal runs. That leaves the push
  unbounded and changes the semantics of a recovery path.

## R2. Single-flight shape
- Decision: a process-wide `Mutex<HashMap<key, slot>>`, plus one
  `tokio::spawn` leader that owns the slot until it exits, with a follow-up
  flag.
- Why: the `coordinator-nfs-load` page. Guards held by the request future are
  released on cancel while git keeps running.
- Alternative rejected: a per-key `tokio::Mutex` awaited by the caller. Callers
  would block, and cancelled callers would free the lock.

## R3. Killing the process group
- Decision: `tokio::process::Command::process_group(0)`. Save the pid at
  spawn. On timeout, `nix::sys::signal::killpg(pid, SIGKILL)`, then `wait()`.
  After any exit, `killpg` again and ignore ESRCH, to catch stragglers such as
  `pack-objects`.
- Alternative rejected: `kill_on_drop` alone, which kills only the direct
  child (see the `worker-execution-hardening` page).
- `nix` is already a dependency of `utils`. `workspace-manager` reaches it by
  re-exporting a small helper from `utils::process`, so no new dependency is
  added.

## R4. Classification
- Deterministic stderr markers: `remote rejected`, `[rejected]`,
  `checked out`, `non-fast-forward`, `fetch first`,
  `does not appear to be a git repository`, `src refspec`,
  `not a git repository`.
- Everything else is transient. The pre-flight live-holder check produces a
  deterministic result without spawning.

## R5. Stale-registration rule
- Decision: an entry is stale only if it is not `locked` and one of these
  holds:
  - (a) the worktree directory is definitely absent and its grandparent is
    definitely present (the mount guard); or
  - (b) the worktree's `.git` resolves to a different admin dir.

  Any I/O error means indeterminate, which means keep.
- Alternative rejected: `git worktree prune`. It has no mount guard and acts
  on all entries at once. It was deliberately removed from the codebase.

## R6. UI surface
- Decision: `RepoBranchStatus.source_mirror_error`, mapped through
  `deriveRepoInfos` → `RepoInfo` → `RepoCard`. The explorer survey found no
  generic warnings channel. `RepoCard` is the per-repo spot, and it is polled
  every 15 s.

## R7. Large-commit measurement
- Decision: after the commit, run
  `git diff-tree -r --root --no-commit-id --diff-filter=A HEAD`. Count the
  lines and sum the sizes with `git cat-file --batch-check='%(objectsize)'`.
  Warn only.
