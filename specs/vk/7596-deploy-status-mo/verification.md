# Verification: Mobile Deploy Status

## Results

| Check | Result |
| --- | --- |
| `pnpm install --frozen-lockfile` | Passed |
| `pnpm run generate-types` | Passed; regenerated `shared/types.ts` from Rust source |
| `pnpm run generate-types:check` | Passed |
| `pnpm --filter @vibe/remote-web test -- src/app/layout/Navbar.test.tsx` | Passed; remote-web suite ran 6 files / 42 tests, including 9 new deploy-status/navbar tests |
| `pnpm run check` | Passed, including local/remote/web-core/UI TypeScript and both Rust workspaces |
| `pnpm run lint` | Passed, including ESLint, Clippy with warnings denied, and unused-i18n validation |
| `pnpm run format` | Passed after the required locked install |
| `pnpm --filter @vibe/ui run format` | Passed (the root formatter does not currently include this package) |
| `bash -n local-build.sh` | Passed |
| `git diff --check -- ':!shared/types.ts'` | Passed; the generated `UserSystemInfo` line retains the generator's existing trailing-space style and is validated by `generate-types:check` |

## Covered behavior

- Production SHA links to the exact GitHub commit.
- Compact age formatting covers current minute, minutes, hours, days, and weeks.
- Future timestamps clamp safely; malformed timestamps produce no age.
- The displayed age advances on the minute timer.
- `dev` is non-linking and does not fabricate an age.
- Missing timestamps retain revision identity without an empty control.
- Mobile `Navbar` renders deploy identity alongside existing Settings and Command bar controls.
- Responsive classes retain SHA while hiding elapsed age below 390 CSS pixels.
