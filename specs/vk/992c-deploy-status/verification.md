# Verification: Desktop Deploy Status

## Results

| Check | Result |
| --- | --- |
| `pnpm install --frozen-lockfile --force` | Passed; `--force` was needed to replace a stale partial patched-package install in the worktree. |
| `pnpm --filter @vibe/web-core test -- src/pages/workspaces/RightSidebar.test.tsx` | Passed; the package suite ran 36 files / 280 tests. |
| `pnpm --filter @vibe/remote-web test -- src/app/layout/Navbar.test.tsx` | Passed; the package suite ran 6 files / 45 tests, including shared deploy-status and mobile navbar regression coverage. |
| `pnpm --filter @vibe/web-core check` | Passed. |
| `pnpm --filter @vibe/remote-web check` | Passed. |
| `pnpm --filter @vibe/local-web check` | Passed. |
| `pnpm run generate-types:check` | Passed; generated API types remain current. |
| `pnpm run lint` | Passed, including frontend ESLint, unused-i18n validation, workspace Clippy, and remote-workspace Clippy with warnings denied. |
| `pnpm run format` | Passed after locked dependency setup. |
| `pnpm --filter @vibe/ui run format` | Passed; the root formatter does not currently include this package. |
| `git diff --check` | Passed. |

## Covered behavior

- The fixed deploy-status row is the first child of the desktop drawer's
  section stack and has intrinsic `flex-none` / `shrink-0` sizing.
- The row has no disclosure button or persisted visibility preference.
- The desktop mount enables the row; the mobile Git-tab reuse of `RightSidebar`
  does not, avoiding duplication with the existing mobile header status.
- Desktop age remains visible at drawer widths while mobile retains its current
  narrow-viewport hiding rule.
- Production revision linking, elapsed-time updates, `dev`, missing timestamp,
  and malformed timestamp behavior remain covered.
