# Technical spec — Serialize and time-bound the workspace branch push to the source repo (VAS-1140)

Task id: `vk/2b38-vas-1140-seriali`

## Root cause (as found in code)

`SharedRepositoryStore::mirror_branch_back`
(`crates/workspace-manager/src/shared_repository.rs`) runs

```
git -C <shared>/repositories/<repo-id> -c core.hooksPath=/dev/null \
    push /srv/src/<repo> refs/heads/<branch>:refs/heads/<branch>
```

through `GitCli::push` → `git_network` → `git_impl_full`, which is a
**synchronous `Command::wait_with_output()` with no timeout**, called inline from
the `async fn ensure_impl`.

`ensure_impl` runs on every `ensure` / `ensure_for_heal`. For a cluster
workspace (placement ≠ `Local`), `ensure_container_exists`
(`crates/local-deployment/src/container.rs`) calls `heal_cluster_worktree` for
every repo on **every call**, and `ensure_container_exists` is called by about 29
request handlers (follow-ups, diffs, git status, PR, attachments …). Every UI
interaction with the session therefore started one more push in the request
future:

- No single-flight: concurrent requests each spawn their own `git push`.
- No timeout: a 1.6 GB pack on NFS takes over an hour, so the processes pile
  up. The request (for example "Send follow-up") hangs until its push ends,
  which is why the UI sat at "Sending".
- It blocks a tokio worker thread (synchronous wait inside an async fn), so the
  runtime and DB pool starve. That produced the `pool timed out` errors.
- Failures are swallowed at `debug!`.
- A deterministic rejection (`branch is currently checked out`) is retried on
  every request, forever.

Why the push was rejected: `SharedRepositoryStore::adopt` re-links a
pre-cluster worktree from `/srv/src/<repo>/.git/worktrees/<name>` to the store.
It rewrites only the worktree's `.git` pointer and the **store's** admin dir.
The **old admin dir in the source checkout is left behind**, still records
`HEAD: ref: refs/heads/<branch>`, and keeps holding the branch. When the
workspace is later deleted, cleanup runs against the store, so the source-repo
entry is never removed. It turns "prunable" and still blocks pushes to that
branch. That is how 120 stale entries built up in `/srv/src/homelab`.

## Design

### 1. `SourceMirror` — single-flight, bounded, backed-off push (new module)

New file `crates/workspace-manager/src/source_mirror.rs`, re-exported from
`lib.rs`.

- A process-wide registry `SOURCE_MIRROR: LazyLock<SourceMirror>` holds a
  `std::sync::Mutex<HashMap<MirrorKey, MirrorSlot>>`, where
  `MirrorKey = (destination path, branch)`. The destination is the operator
  checkout, which is where the contention happens.
- `SourceMirror::request(store, dest, branch) -> RequestOutcome` is
  **non-blocking** and never awaits git:
  - If a push for the key is in flight, set `rerun = true` and return
    `Coalesced`. No new process is started.
  - If the slot is in backoff (`now < next_attempt_at`), return
    `BackingOff`.
  - Otherwise mark the slot in flight and `tokio::spawn` a leader task, then
    return `Started`. The leader owns the slot. A cancelled request future
    cannot release it while git keeps running (see the
    `coordinator-nfs-load` knowledge page).
- The leader task, per round:
  1. Read the store tip (`rev-parse --verify refs/heads/<branch>^{commit}`) and
     the destination tip, both time-bounded. If they are **equal**, record
     success and stop. No push happens in the common steady state, which
     removes almost all of the 1–3 minute pushes.
  2. If the previous failure was **deterministic** and the store tip equals
     the tip that failed, skip without spawning (`Unchanged`). A
     deterministic error is retried only when the branch moves **and** the
     backoff has elapsed.
  3. **Pre-flight on the destination** (filesystem only, no git spawn): walk
     `<dest common dir>/worktrees/*`. For each entry whose `HEAD` is
     `ref: refs/heads/<branch>`:
     - If it is a *stale registration* (see §3): remove it, then continue.
     - If it is a live worktree: deterministic failure
       `branch '<b>' is checked out at <path> in <dest>`. Do not push.

     If the destination's main worktree has the branch checked out, that is
     also a deterministic failure.
  4. Run the push via `tokio::process::Command` with `process_group(0)`,
     `kill_on_drop(true)`, `stdin(null)`, `GIT_TERMINAL_PROMPT=0`,
     `-c core.hooksPath=/dev/null`, and stderr captured. Wrap it in
     `tokio::time::timeout(push_timeout)`. On expiry, `killpg(SIGKILL)` the
     group, so `git pack-objects` children die with the parent, then reap the
     child and record a `Timeout` failure.
  5. Classify the result:
     - `remote rejected`, `checked out`, `non-fast-forward`, `[rejected]`,
       `fetch first`, `does not appear to be a git repository`, and
       `src refspec … does not match` are **deterministic**.
     - Anything else is **transient**.
     - Expiry is **timeout**.
  6. On failure: `warn!` with repo, branch, kind, attempt count, the next retry
     time and a bounded stderr excerpt (last 20 lines, at most 2 KB). Store it
     on the slot. The backoff is `base * 2^(attempts-1)`: base 60 s, cap
     30 min for transient and timeout, cap 6 h for deterministic.
  7. On success after a failure: log `info!` "recovered", then clear the
     failure.
  8. If `rerun` was set during the round and the round succeeded, run one more
     round. Requests that arrive while a push runs collapse into at most one
     follow-up push. After a failure the rerun is dropped and backoff governs.
- Configuration:
  - `VK_SOURCE_MIRROR_PUSH_TIMEOUT_SECS` (default 300) sets the push timeout.
  - `VK_SOURCE_MIRROR_DISABLED=1` turns off the courtesy mirror altogether.
    `ensure_impl` treats the mirror as advisory already.
- Read API: `SourceMirror::failure(dest, branch) -> Option<SourceMirrorFailure>`
  returns `{ kind, message, attempts, last_attempt_at, next_attempt_at }` for
  the UI.
- Test hooks: `SourceMirror::new(config)`, so tests use a private instance
  with a short timeout. `wait_idle(key)` lets tests await the leader.

`mirror_branch_back` keeps its local-head-only guard and then calls
`SOURCE_MIRROR.request(...)` instead of `cli.push`. Nothing in `ensure_impl`
waits on git for the mirror any more.

### 2. Show the failure on the workspace

Add an optional per-repo field, `source_mirror_error`, carrying
kind + message + next retry, to the existing per-workspace-repo status response
that the workspace UI already renders. The exact struct is chosen in the
implementation plan from the codebase survey. The handler reads it from
`SOURCE_MIRROR.failure(repo.path, workspace.branch)`. It is an in-memory
signal: it disappears on restart, and the next attempt recreates it if the
cause persists. The frontend shows a compact warning row with the message.

### 3. Stale worktree registrations in the source repo

Add to `utils::worktree_linkage` a filesystem-only scanner, `stale_registrations(common_dir)`,
plus `remove_registration(admin_dir)`. An entry `<common>/worktrees/<name>` is
**stale** only if all of these hold:

- there is no `locked` file;
- `gitdir` is readable and names `<wt>/.git`;
- **and** either:
  - (a) `<wt>` definitely does not exist (`try_exists == Ok(false)`) **and**
    `<wt>`'s grandparent definitely exists. This guard proves the storage is
    mounted, so a missing NFS mount never reads as "deleted"; or
  - (b) `<wt>/.git` exists but resolves to a *different* admin dir. The
    worktree was re-linked (adopted) elsewhere, so this entry is an orphan
    holding the branch.

Any read or stat error means **keep**, following the fail-safe rule in
`workspace-directory-reclamation`.

Call sites:

- **`adopt`**: before rewriting `.git`, remember the old admin dir. After the
  adoption is verified `Portable`, remove that old admin dir if it is in a
  different common dir and its `gitdir` names this worktree. This is the move
  case.
- **Workspace delete** for a clustered workspace: after the store-side
  cleanup, for each repo, remove entries in `repo.path`'s common dir whose
  `gitdir` names the deleted worktree's `.git`. This is the delete case,
  limited to that one entry.
- **Startup**: a background, best-effort sweep runs over every registered
  repo's common dir and removes the stale entries it finds (rule above). Each
  repo is bounded by a timeout, and the sweep logs a count per repo. This
  clears the existing 119 entries in `/srv/src/homelab`.
- **Push pre-flight** (§1.3) uses the same rule for entries that hold the
  branch.

No `git worktree prune` subprocess is used. Its notion of "prunable" does not
include the mount guard (a), and it prunes everything at once.

### 4. Large-commit guard (optional item; warn only)

The agent auto-commit path counts files added and bytes in the commit it just
made (`diff-tree --numstat` / `--name-status` limited, or `git diff --cached
--stat` before commit). It emits a `warn!` naming the workspace, the counts and
the thresholds `VK_LARGE_COMMIT_WARN_FILES` (default 5000) and
`VK_LARGE_COMMIT_WARN_MB` (default 200). It warns only and refuses nothing,
because refusing an agent's commit loses no data but blocks the turn in a way
the operator did not choose. The final scope is decided in the plan, after
locating the auto-commit code.

## Acceptance mapping

| Acceptance | Mechanism | Test |
| --- | --- | --- |
| At most one push per branch | slot `in_flight` + `rerun` coalescing | spawn N requests against a destination whose push is blocked; assert exactly 1 live process |
| A hung push is killed with its children within the timeout | `process_group(0)` + `killpg(SIGKILL)` | `GIT_SSH`-free hang: a destination with `receive.procReceiveRefs` is not portable, so instead use a fake `git` wrapper (`PATH` override) that `sleep`s in a child; assert the call returns within timeout + ε and the child pid is gone |
| A rejected push is logged and shown, with no tight loop | classification + backoff + tip gate | destination with the branch checked out; assert `Deterministic`, then that a second request is `BackingOff`/`Unchanged` and spawns nothing |
| Deleting a workspace leaves no prunable entry | delete-path targeted removal + adopt cleanup | unit tests on the scanner; adopt test asserts the old admin dir is gone |

## Non-goals

- Changing when `heal_cluster_worktree` runs.
- Removing the mirror altogether (it exists for rollback).
- Persisting mirror failures to SQLite.
