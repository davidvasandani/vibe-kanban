# Technical Spec: sg-monorepo `start_workspace` never creates worktree/session

Task: VAS-981. Branch: `vk/0fab-vas-981-sg-monor`.

## Root causes (host-confirmed, issue comments 2026-11-05 13:55/13:57 UTC)

Two independent, host-verified failures, plus one false alarm:

1. **False alarm — async creation is just slow for a large repo.** sg-monorepo
   worktree checkout over NFS takes ~1 minute; `start_workspace` returns the
   workspace id immediately (creation runs in a detached `tokio::spawn`,
   `crates/server/src/routes/workspaces/create.rs:502`), so polling
   `list_sessions` within that window looks like "no session ever appears"
   but isn't. No code fix needed for this part; see "Non-fixes" below.

2. **Branch missing from local clone is a hard, swallowed failure.**
   `add_repository` (`crates/workspace-manager/src/workspace_manager.rs:244`)
   calls `GitService::check_branch_exists`, which only looks at local and
   local remote-tracking refs — it never fetches. A branch that exists only
   on `origin` (e.g. a PR branch nobody has started a workspace from) fails
   with `WorkspaceError::BranchNotFound`, confirmed in host logs:
   `Branch 'feat/linux-pos-prototype' does not exist in repository 'sg-monorepo'`.
   The failure is recorded correctly (`creation_status = 'failed'`,
   `crates/server/src/routes/workspaces/create.rs:560-571`) but:
   - the stored `creation_error` was a **hardcoded generic string**, not the
     real error — the actual diagnosis was logged server-side only, never
     reaching the DB row or any caller;
   - no caller (`create_session`, the MCP tool) ever looked at
     `creation_status`/`creation_error`, so downstream calls got a plain
     "Workspace not found" (400) or a bare 404, which doesn't explain anything.

3. **A repository-managed git hook broke every `git worktree add` on the
   shared clone.** Some sg-monorepo workspace ran the repo's own `lefthook
   install` (root `postinstall`), writing `post-checkout`/`post-merge`/
   `pre-commit`/`pre-push` shims into the **shared clone's common git dir**
   (`crates/git/src/cli.rs`'s `git_impl` runs every vk git command with
   `-C <repo_path>`, i.e. directly against that common dir — vk never ran
   with hooks disabled). The `post-checkout` hook referenced a path from one
   workspace's `node_modules` and failed under `/bin/sh` (`. .lefthookrc`
   searches `$PATH`, not cwd), so it exited non-zero on every subsequent
   `git worktree add`, which git itself then rolled back. Every new
   sg-monorepo (and vibe-kanban) workspace would fail from 13:45:39Z until a
   human manually moved the four hooks aside (done at 13:57Z, see comment).
   vk had no durable protection against this: nothing disables hooks for its
   own git plumbing, and nothing stops a workspace's own tooling from writing
   into the shared, cross-workspace common git dir.

## Fixes implemented

1. **`crates/git/src/cli.rs` (`GitCli::git_impl`)** — every git command vk
   runs now passes `-c core.hooksPath=/dev/null`. This is vk's own git
   plumbing (worktree add/remove/move/repair, checkout, merge, rebase,
   commit, fetch, push, diff/status) acting on the user's behalf, never an
   interactive session, so a repository's hooks must never be able to block
   it — regardless of what any workspace's own postinstall writes into the
   shared common git dir. (Matches the existing precedent in
   `crates/remote/src/github_app/service.rs`, now centralized instead of
   applied ad hoc per call site.)

2. **`crates/git/src/lib.rs` (`GitService::fetch_remote_branch`, new) +
   `crates/workspace-manager/src/workspace_manager.rs` (`add_repository`)** —
   when the target branch doesn't exist locally, resolve the repo's default
   remote and check (via `git ls-remote`-backed `check_remote_branch_exists`,
   no local mutation) whether the branch exists there. If it does, fetch
   just that branch (`+refs/heads/<b>:refs/remotes/<remote>/<b>`) and retry
   the local check before failing. A branch that is also absent on the
   remote still fails with the original `BranchNotFound`; a fetch failure
   (auth/network) now propagates as a distinct `GitService` error instead of
   being masked as "branch not found".

3. **`crates/server/src/routes/workspaces/create.rs`** — the background
   creation task now records the *real* error's `Display` text as
   `creation_error` instead of a hardcoded placeholder, so a failed
   workspace's row says why it failed (branch missing, hook failure, etc.),
   inspectable without host log access.

4. **`crates/server/src/routes/sessions/mod.rs` (`create_session`)** — now
   checks `workspace.creation_status` after confirming the row exists:
   `Ready` proceeds as before; `Failed` returns the stored `creation_error`
   instead of creating a session against a workspace with no worktree;
   `Queued`/`Running` returns "still being created, retry shortly" instead of
   racing ahead of the background task.

## Non-fixes / explicitly out of scope

- **Workspace-level "slow but healthy" creation** is not a bug (point 1
  above) — no change to the fire-and-forget creation flow's timing.
- **The live `restart_workspace` MCP tool's exact route/handler could not be
  located in this checkout** (`crates/mcp/src/task_server/tools/mod.rs` only
  registers `list/update/delete_workspace`), yet it responded live against
  the running instance during investigation. This suggests version skew
  between this checkout and the deployed server, or an orchestrator-scoped
  tool implemented outside the paths searched. Not fixed here — flagged for
  the user, since any fix would be aimed at the wrong code without first
  confirming where that tool actually lives in the deployed version.
- **The shared clone's hooks directory itself** was already remediated
  manually on the host (hooks moved to
  `repositories/ffd45a4d-…/hooks-backup-lefthook-20261105/`, approved by
  David 2026-11-05 13:57 UTC). This spec only adds the durable code-side
  protection (`core.hooksPath=/dev/null`) so recurrence (e.g. another
  workspace re-running `lefthook install`) can't break worktree creation
  again; it does not touch the host filesystem.
- **`delete_workspace`/`restart_workspace` 404 behavior**: the current
  checkout's `delete_workspace` (`crates/server/src/routes/workspaces/core.rs`,
  via `load_workspace_middleware` → `Workspace::find_by_id`) is a plain
  DB-by-id lookup with no worktree-existence precondition, so it already
  supports deleting a workspace whose worktree was never created. The bare
  404s observed live against workspace `028b9988…` are therefore most likely
  explained by that row no longer existing in the DB by the time it was
  probed (consistent with `create_session`'s 400 on the same id), not by a
  lookup bug in this code. No change made here; flagged as an open item if
  a reproducible case of "list shows it, delete 404s on the same id, in the
  same moment" turns up.

## Branch rebase (2026-11-05, mid-implementation)

This branch was discovered to be cut from a stale base (390 commits behind
`origin/main`, carrying 30 unrelated "cluster node diagnostics" commits —
not VAS-981 work). Per David's decision (issue comment 14:24 UTC), it was
reset to `origin/main` at `182a0cb9` (never pushed, so no shared history was
touched) and the fixes above were reapplied against current `main`, which
had independently gained a `GitHubCredentials` parameter on the
remote-branch-check/fetch path (a GitHub App org-credentials feature,
`#392`) since this branch's original base. The fetch-on-miss fix now passes
`GitHubCredentials::default()` — the same "use the server's existing
fallback credential" behavior this code path always had before `#392`,
since the repo is already registered and fetchable and this fix doesn't
need the newer per-owner org-token resolution. The constitution's principle
numbering also moved on (LVIII at rebase time); the new principle for this
task is **LIX**, not XXXI as originally drafted.

## Separate bug noticed during this investigation (not fixed here)

David flagged, for awareness in the PR rather than as part of this fix: on
SWE-400, an agent's `create_issue_comment` call with `intent:
reply_to_agent` delivered the comment back to **its own** session,
attributed to David (comment `9d578ea5-608f-4c25-b6fa-876ed96430e1`, session
`98f90418-dc05-4299-beec-3ee9770d4b0d`). `reply_to_agent` from the linked
session itself should not self-deliver. Out of scope for this change; not
investigated or fixed here.

## Acceptance criteria mapping

- Root cause identified for the three orphan ids: done (comments above);
  `028b9988`/`e0883eae` = branch-miss, `0c9bde35`/`b90f77b2` = hook failure,
  `caa20220`/`0fab86e1` = healthy (just slow).
- `start_workspace` surfaces failures instead of a bare id: partially — the
  real error is now recorded on the row and surfaced the next time anything
  reads the workspace (`create_session`); the initial HTTP/MCP response is
  still asynchronous by design (see "Non-fixes").
- A failed creation leaves a record `delete_workspace` can remove: already
  true in this checkout (see above); unaffected by these changes.
- Remote-only branch works or fails clearly: fixed (fetch-on-miss).
- Verification: covered by the targeted fetch-on-miss fix plus manual host
  remediation; a live re-run against sg-monorepo/`feat/linux-pos-prototype`
  is still recommended once this branch is deployed.
