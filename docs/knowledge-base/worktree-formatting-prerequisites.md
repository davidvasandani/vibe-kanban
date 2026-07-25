# Worktree-safe formatting prerequisites

Tags: `7243-make-frontend-fo`

## Boundary rule
Multi-stage repository verification must validate dependencies before its first
mutating stage. In Vibe Kanban, root formatting runs Rustfmt before three
filtered Prettier scripts. Without a preflight, a fresh worktree can modify Rust
files and only then reveal that frontend dependencies are absent.

Use the root `preformat` lifecycle hook as the boundary:

1. Check the package-local formatter executable for every frontend package in
   the root format chain.
2. Gather all missing packages instead of stopping at the first one.
3. Fail before Rustfmt with the exact recovery command:
   `pnpm install --frozen-lockfile`.
4. Keep the existing format script and package format commands unchanged, so a
   successful result still proves every stage ran.

Avoid installing dependencies implicitly inside `format`. An implicit install
adds network and lockfile side effects to a formatting command and obscures
registry/setup failures. A frozen, explicit setup command is predictable in
both contributor and agent workflows.

## pnpm executable location
Prettier is declared by the frontend workspace packages, not the root package.
With pnpm's isolated linker, the authoritative shims are:

- `packages/web-core/node_modules/.bin/prettier`
- `packages/local-web/node_modules/.bin/prettier`
- `packages/remote-web/node_modules/.bin/prettier`

Do not assume `node_modules/.bin/prettier` exists at the repository root.
Windows uses `prettier.cmd`, so select the platform suffix in the checker.

## Regression pattern
Test prerequisite detection with temporary fixture roots rather than deleting
the active worktree's `node_modules`. Cover:

- no formatter shims;
- every required shim;
- a partial installation, proving no package is silently skipped.

Also include the checker and its test paths in CI change filters. Adding a test
command to a filtered job is insufficient if changes to the tested files do not
trigger that job.

## Verification
Exercise both contracts:

- Before installation, `pnpm run format` exits during `preformat`, includes the
  frozen install command, and emits no `backend:format`, `cargo fmt`, or opaque
  `prettier: command not found` output.
- After `pnpm install --frozen-lockfile`, `pnpm run format` completes backend,
  web-core, local-web, and remote-web formatting.
