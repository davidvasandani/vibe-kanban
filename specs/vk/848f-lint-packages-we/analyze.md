# Analyze: spec ↔ plan ↔ tasks ↔ constitution

- **warning** (tasks): `AGENTS.md` (the repo `CLAUDE.md`) says `pnpm run lint`
  "runs web/ui ESLint". After this change it also covers web-core and
  remote-web. Added T028 to update that line.
- **warning** (tasks): the rename of `settingsRegistry.tsx` leaves stale
  filenames in `wiki/external-connector-sync.md` and
  `docs/knowledge-base/remote-external-integrations.md`. Added T029. Other
  wiki and docs mentions of moved modules name the component, not a path, so
  they stay accurate.
- **info** (plan §5): `remote-web/src/test/CreateModeRepoPickerBar.test.tsx`
  may `vi.mock` relative or aliased sibling paths of the moved component.
  T008 has to update the mock specifiers along with the import, or the mocks
  silently stop applying. T025 (the vitest run) catches a miss.
- **info** (spec FR-6 ↔ issue): the issue allowed justified inline disables,
  but the shared `eslint-comments/no-use` rule bans them. The spec resolves
  this in favour of the shared rule (constitution XLVIII). Consistent.
- **info** (plan §1): `pnpm-lock.yaml` also dedupes `debug` (4.4.1→4.4.3) and
  `acorn` (8.15→8.16) within existing ranges, as a side effect of adding the
  remote-web importer. No new packages. Recorded in the PR body.
- **info** (constitution XLVIII, X, IV, XIV): no violations. Every
  `exhaustive-deps` site has its own justification in plan §7. McpServerDialog
  keeps the open-only re-seed.
- No coverage gaps: FR-1…FR-9 map to T002–T006, T024–T027. FR-6 and FR-7 map to
  T007–T023.
