# Verification: Commits Behind in the Git Header

**Run:** 2026-08-13

## Passed

- `pnpm install --frozen-lockfile`
- `pnpm exec vitest run src/pages/workspaces/GitBehindHeader.test.tsx src/pages/workspaces/RightSidebar.test.tsx`
  from `packages/web-core`: 2 files, 8 tests passed.
- `pnpm run format`: completed across all required Rust and web workspaces.
- `pnpm run check`: local-web legacy guard, local-web, remote-web, web-core,
  UI, root Rust workspace, and remote Rust workspace checks completed
  successfully.
- `pnpm exec tsc --noEmit` from `packages/web-core`.
- `git diff --check`.

## Repository baseline failure

`pnpm run lint` passes the local-web and UI ESLint phases, then fails in the
backend Clippy phase on the pre-existing function
`crates/server/src/routes/workspaces/create.rs:297`:

```text
error: this function has too many arguments (8/7)
```

The task does not modify that file or workspace-creation behavior. Fixing it
would be unrelated expansion, so it is recorded rather than folded into this
Git-header change. The changed web-core files pass TypeScript compilation and
the repository has no standalone web-core ESLint configuration/script.
