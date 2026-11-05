# Implementation Plan: VAS-981

1. **Disable hooks for vk's own git plumbing.**
   `crates/git/src/cli.rs`, `GitCli::git_impl`: add `-c core.hooksPath=/dev/null`
   to every invocation, at the single choke point all `GitCli` commands pass
   through. Follows the existing precedent in
   `crates/remote/src/github_app/service.rs`, now centralized.

2. **Fetch a branch that only exists on the remote, instead of failing
   immediately.**
   - `crates/git/src/lib.rs`: add `GitService::fetch_remote_branch`
     (new, public) — fetches one branch into
     `refs/remotes/<remote>/<branch>` via the existing `GitCli::fetch_with_refspec`.
   - `crates/workspace-manager/src/workspace_manager.rs`, `add_repository`:
     on a local `check_branch_exists` miss, resolve the default remote, check
     `check_remote_branch_exists` (no mutation), and if true, fetch then
     re-check before raising `BranchNotFound`.

3. **Stop swallowing the real creation failure.**
   `crates/server/src/routes/workspaces/create.rs`,
   `create_and_start_workspace`'s background task: replace the hardcoded
   `fail_creation_with_retry` message with `error.to_string()` so
   `creation_error` on the workspace row reflects what actually failed.

4. **Make `create_session` respect `creation_status`.**
   `crates/server/src/routes/sessions/mod.rs`, `create_session`: after the
   existing `find_by_id` check, branch on `workspace.creation_status`:
   `Ready` → proceed; `Failed` → return the stored `creation_error`;
   `Queued`/`Running` → return a "still being created" message. Prevents a
   session from being created against a workspace with no worktree, and
   replaces the opaque "Workspace not found" with the real reason.

5. **Verify.**
   - `cargo check -p git -p workspace-manager -p server` — done, clean.
   - `cargo fmt`, `cargo clippy` for touched crates.
   - `cargo test` for `git`, `workspace-manager`, `server` (sessions/workspaces
     modules) — no existing unit tests cover `add_repository` or
     `git_impl`'s hooks behavior; add targeted tests if a reasonable one is
     feasible without live network/remote fixtures.
   - Independent review via the `codex-review` skill (pipeline stage 11).

6. **Explicitly not touched** (see technical-spec.md "Non-fixes"): the
   shared clone's hooks directory (already remediated manually on the host),
   `delete_workspace`/`restart_workspace` 404 behavior (no bug found in this
   checkout), and the async/eventually-consistent nature of workspace
   creation itself.

7. **Knowledge base + PR** (pipeline stages 12-13): record what shipped,
   then open a PR against `main`; pause for explicit confirmation before
   merging.
