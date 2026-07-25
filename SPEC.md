# Technical Specification: Fresh-Worktree Formatting Preflight

## Goal
Make `pnpm run format` reliable and understandable in a fresh Vibe Kanban
development worktree while preserving all existing Rust and frontend formatting
stages.

## Behavior

### Ready worktree
After:

```bash
pnpm install --frozen-lockfile
```

`pnpm run format` runs, in order:

1. `pnpm run backend:format`
2. `pnpm run web-core:format`
3. `pnpm run local-web:format`
4. `pnpm run remote-web:format`

### Missing or incomplete dependency installation
Before any formatting stage begins, `preformat` checks for the package-local
Prettier executable used by:

- `packages/web-core`
- `packages/local-web`
- `packages/remote-web`

If any executable is missing, the command exits non-zero, identifies every
affected package, and directs the user to run:

```bash
pnpm install --frozen-lockfile
```

It must not run Rustfmt, emit `prettier: command not found`, or silently skip a
frontend package.

## Implementation
- `scripts/check-format-prerequisites.mjs` owns the preflight and exposes
  reusable functions for tests.
- The root `preformat` package lifecycle hook invokes the checker without
  changing the existing `format` script.
- `scripts/check-format-prerequisites.test.mjs` uses Node's built-in test runner
  and temporary fixture workspaces to verify absent, complete, and partial
  dependency states.
- The frontend CI job runs the regression test after dependency installation.

## Compatibility and constraints
- Node.js 20+ and pnpm 8+ remain the system-level prerequisites.
- The dependency installation is explicit; formatting does not perform network
  or lockfile mutations.
- The checker accounts for Windows `.cmd` executable shims.
- No formatting rule, package dependency, generated source, or formatter stage
  changes.

## Verification
- Run `pnpm run test:format-prerequisites`.
- In a dependency-free worktree, verify `pnpm run format` exits in `preformat`
  with the frozen-lockfile setup command and no `backend:format` output.
- Run `pnpm install --frozen-lockfile`, then `pnpm run format`, and confirm all
  four existing stages complete.
