# Implementation plan — VAS-1140

Builds on `technical-spec.md` and `prior-knowledge.md`.

## Step 1. Stale-registration scanner (`crates/utils/src/worktree_linkage.rs`)

1. `pub struct Registration { admin_dir, worktree_path, head_branch: Option<String>, locked: bool }`.
2. `pub fn registrations(common_dir) -> Result<Vec<Registration>, String>`.
   It reads `worktrees/*/{gitdir,HEAD,locked}`. Unreadable entries come back
   as an `Err` (fail-safe), never dropped silently.
3. `pub enum RegistrationState { Live, Stale(reason), Indeterminate(reason) }`.
   `pub fn registration_state(&Registration) -> RegistrationState`:
   - locked → Live.
   - `<wt>/.git` exists and points back to this admin dir → Live.
   - `<wt>/.git` exists and points to another admin dir → Stale("re-linked to X").
   - `<wt>` absent (`try_exists == Ok(false)`) and grandparent present → Stale("worktree directory removed").
   - anything else → Indeterminate.
4. `pub fn remove_stale_registration(&Registration) -> io::Result<bool>`.
   It re-checks the state just before `remove_dir_all`.
5. Unit tests: live, locked, removed, re-linked, missing mount (grandparent
   absent) → Indeterminate, and unreadable gitdir.

## Step 2. `SourceMirror` (`crates/workspace-manager/src/source_mirror.rs`)

1. Config: `push_timeout`, `backoff_base`, `transient_cap`,
   `deterministic_cap`, `disabled`. `from_env()` reads
   `VK_SOURCE_MIRROR_PUSH_TIMEOUT_SECS` and `VK_SOURCE_MIRROR_DISABLED`.
2. A global `LazyLock<SourceMirror>` and `SourceMirror::global()`.
3. `request(store, dest, branch) -> RequestOutcome`, which never blocks.
4. Leader loop (`run_round`):
   - Compare tips (`rev-parse` via the time-bounded runner).
   - Gate on the deterministic tip.
   - Pre-flight with `registrations()`, `registration_state()` and remove.
   - Time-bounded push with process-group kill.
   - Classify the outcome and update the slot.
5. `run_git_bounded(dir, args, timeout) -> Result<Output, BoundedError>`:
   - spawn with `process_group(0)`, `kill_on_drop`, stdin null and
     `GIT_TERMINAL_PROMPT=0`;
   - save the pid;
   - on timeout, `killpg(pid, SIGKILL)` then `wait()`;
   - after a normal exit, `killpg(SIGKILL)` again to catch stragglers, ignoring
     ESRCH.
6. `failure(dest, branch) -> Option<SourceMirrorFailure>`, which is Serialize
   and TS.
7. Tests (private instance, temp repos):
   - in sync → no push and no failure;
   - behind → pushed, and dest == store;
   - branch checked out by a live worktree in dest → Deterministic, the next
     request is BackingOff, and no push happens;
   - prunable/stale registration holding the branch → removed, then the push
     succeeds;
   - a fake `git` on `PATH` → hang killed within the timeout, with its
     grandchild `sleep` dead (pid file check). Use a `git_binary` override on
     the config instead of mutating `PATH`: hermetic and parallel-safe;
   - coalescing: with a slow fake git, 10 requests → exactly 1 started and
     the rest coalesced; the counted spawns (fake git appends to a file) are
     at most 2.
   - deterministic failure + backoff elapsed + same tip → `Unchanged`, with no
     spawn.

## Step 3. Wire into `SharedRepositoryStore`

- `mirror_branch_back` calls `SourceMirror::global().request(&store, &repo.path, branch)`
  and logs the outcome at debug.
- `adopt`: capture the old admin dir from the `.git` pointer before
  `write_linkage`. After the result is `Portable`, if the old admin dir is
  outside the store and its `gitdir` names this worktree, remove it with
  `remove_stale_registration`, which re-checks the state, now re-linked, so it
  is Stale. Log at info.
- New `pub fn forget_source_registrations(repo, worktree_path)` removes the
  source-repo entries whose `gitdir` names `<worktree>/.git` and are Stale.
- Extend the adoption test: assert the old admin dir is gone. It sits in
  `moved-away` in the fixture. Add a fixture where the checkout is still
  reachable.

## Step 4. Delete path (`crates/local-deployment/src/container.rs::cleanup_workspace`)

After a successful `WorkspaceManager::cleanup_workspace`, for clustered inputs
(`git_path != repo.path`), call
`SharedRepositoryStore::forget_source_registrations(&input.repo, &workspace_dir.join(&input.repo.name))`
inside `spawn_blocking`. Best effort: log and move on.

## Step 5. Startup sweep

`crates/local-deployment`: in `spawn_workspace_cleanup`, before the loop, run a
one-time `sweep_source_registrations`:

- `Repo::list_all`;
- for each repo path, `registrations()`;
- skip any entry whose worktree path lies under a live workspace's
  `container_ref` (DB guard);
- remove entries that are Stale;
- log the count per repo at info.

It runs in `spawn_blocking` and is bounded per repo by a timeout (abandoning
the wait, not the thread). It runs whether or not clustering is on, since
operator checkouts get stale entries either way. It only touches entries that
pass the Stale rule plus the DB guard.

## Step 6. UI surface

- `RepoBranchStatus` gets `source_mirror_error: Option<SourceMirrorFailure>`.
  The struct is not flattened into `BranchStatus`. It is filled from
  `SourceMirror::global().failure(&repo.path, &workspace.branch)`.
- Register `SourceMirrorFailure` and `SourceMirrorFailureKind` in
  `generate_types.rs`, then run `pnpm run generate-types`.
- `deriveRepoInfos` maps the field to `sourceMirrorError?: string`, the
  message plus the retry time. `RepoInfo` gets the field, and `RepoCard` gets
  a `sourceMirrorError` prop that renders a small warning line
  (`WarningCircleIcon`, `text-error`, truncating, with the full text in
  `title`).
- Add the i18n key to the `tasks` namespace in all locales (the locale
  consistency check).
- Vitest for `deriveRepoInfos` mapping, if a test file exists. Otherwise add
  `gitPanelRepoInfo.test.ts`.

## Step 7. Large-commit guard (warn only)

- `GitService::commit_size(path) -> (added_files, added_bytes)` uses
  `git diff-tree -r --root --no-commit-id --diff-filter=A HEAD` (raw), then
  sums sizes via `cat-file --batch-check`.
- In `commit_repos`, after a successful commit, if the counts exceed
  `VK_LARGE_COMMIT_WARN_FILES` (default 5000) or `VK_LARGE_COMMIT_WARN_MB`
  (default 200), `warn!` with the workspace path, the counts and the
  thresholds.
- Unit test on `commit_size`.

## Step 8. Verify

```
cargo fmt
cargo clippy -p utils -p workspace-manager -p local-deployment -p server -p git
cargo test -p utils worktree_linkage
cargo test -p workspace-manager
cargo test -p git commit_size
pnpm run generate-types
pnpm run check
pnpm run lint
pnpm run format
```

## Risks

- Moving the mirror to the background means a status poll right after a commit
  can show stale ahead/behind counts until the next poll (15 s). Accepted.
- A tokio runtime is required in `ensure_impl`. It is always async, so this is
  satisfied.
- Global state in tests: each test uses its own temp paths, so the keys are
  distinct.
