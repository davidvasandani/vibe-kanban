# Implementation Plan: Worktree-Safe Formatting

1. Add a Node preflight module at
   `scripts/check-format-prerequisites.mjs`.
   - Check the package-local Prettier shims for `web-core`, `local-web`, and
     `remote-web`.
   - Gather all missing packages in one pass.
   - Exit with `pnpm install --frozen-lockfile` guidance when incomplete.

2. Add regression tests at
   `scripts/check-format-prerequisites.test.mjs`.
   - Use temporary fixture workspaces.
   - Cover no dependencies, all dependencies, and a partial installation.
   - Assert the diagnostic is actionable and never reproduces the opaque
     `prettier: command not found` message.

3. Integrate the check in `package.json`.
   - Register it as `preformat` so pnpm invokes it before the unchanged root
     `format` command.
   - Add `test:format-prerequisites` using Node's built-in test runner.

4. Add the focused regression test to
   `.github/workflows/test.yml` after the existing dependency install and before
   concurrent frontend checks.

5. Update setup guidance.
   - Make `pnpm install --frozen-lockfile` the documented fresh-worktree command
     in `README.md`.
   - Add the same requirement to `AGENTS.md` and its task-completion checklist.

6. Verify the failure path before installing dependencies.
   - Run `pnpm run test:format-prerequisites`.
   - Run `pnpm run format` with no worktree `node_modules`.
   - Confirm it exits during `preformat`, prints the setup command, and emits no
     `backend:format`, `cargo fmt`, or `prettier: command not found` output.

7. Verify the ready-worktree path.
   - Run `pnpm install --frozen-lockfile`.
   - Run `pnpm run format`.
   - Confirm backend, web-core, local-web, and remote-web formatting all
     complete.

8. Run `git diff --check`, independently review the full diff, address
   significant findings, and repeat focused/full verification as needed.

9. Record the reusable preflight pattern in
   `docs/knowledge-base/worktree-formatting-prerequisites.md`, tag it with
   `7243-make-frontend-fo`, and refresh the knowledge-base index.
