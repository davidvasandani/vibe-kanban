# Verification: Remote-mainline workspace defaults

All checks passed on 2026-08-10.

- `pnpm install --frozen-lockfile`
- `pnpm --filter @vibe/web-core exec vitest run src/shared/hooks/useRepoBranchSelection.test.tsx` (3 tests)
- `pnpm --filter @vibe/web-core check`
- `pnpm run lint`
  - local and shared frontend ESLint
  - primary Cargo workspace clippy with `qa-mode`
  - remote Cargo workspace clippy
  - unused i18n-key validation
- `pnpm run format`
- `git diff --check`

The hook-level regression proves that a registered checkout on a local
deployment branch emits the exact `origin/main` target. It also proves that a
valid configured default, a valid explicit initial branch, and a subsequent
manual override retain their precedence. Existing canonical-helper tests cover
`origin/master`, current/first fallback, and empty branch lists.
