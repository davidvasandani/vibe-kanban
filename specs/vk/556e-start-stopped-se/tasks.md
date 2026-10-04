# Tasks: Start stopped Session UI

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent
files and may run in parallel within their group. Each task names the
file(s) it changes.

## Phase 1: Setup
- [x] T001 Ensure worktree dependencies are installed (`pnpm install --frozen-lockfile` at repo root; changes no tracked files)

## Phase 2: Core
- [x] T002 Extract the footer icon-button group into a local `renderIconButtons()` closure and gate `footerLeft` on `!interruptedNotice`, in `packages/ui/src/components/SessionChatBox.tsx` (depends on T001)
- [x] T003 Replace the interrupted banner's message/Resume row with `renderIconButtons()` + status label + restart button, in `packages/ui/src/components/SessionChatBox.tsx` (FR-001, FR-003) (depends on T002)
- [x] T004 [P] Add `status`/`restart`/`restarting` keys and remove `resume`/`resuming` from `conversation.interrupted` in `packages/web-core/src/i18n/locales/en/tasks.json` (FR-005) (depends on T003)
- [x] T005 [P] Mirror T004 in `packages/web-core/src/i18n/locales/es/tasks.json` (depends on T003)
- [x] T006 [P] Mirror T004 in `packages/web-core/src/i18n/locales/fr/tasks.json` (depends on T003)
- [x] T007 [P] Mirror T004 in `packages/web-core/src/i18n/locales/ja/tasks.json` (depends on T003)
- [x] T008 [P] Mirror T004 in `packages/web-core/src/i18n/locales/ko/tasks.json` (depends on T003)
- [x] T009 [P] Mirror T004 in `packages/web-core/src/i18n/locales/zh-Hans/tasks.json` (depends on T003)
- [x] T010 [P] Mirror T004 in `packages/web-core/src/i18n/locales/zh-Hant/tasks.json` (depends on T003)

## Phase 3: Validation

The knowledge-base update, Codex review and PR are pipeline stages 11–13, not SpecKit tasks.

- [x] T011 Add Vitest coverage in `packages/remote-web/src/test/SessionChatBox.test.tsx` (depends on T003, T004–T010):
  - not interrupted: icon buttons render in the footer, no restart button
  - interrupted: icon buttons render once (in the banner, not the footer), a restart button is present (not "Resume"), clicking it calls `onResume`
- [x] T012 Run `node scripts/check-unused-i18n-keys.mjs`, a manual locale key-set diff, `pnpm --filter @vibe/ui run check`, `pnpm --filter @vibe/ui run lint`, and the new Vitest file; fix any fallout (depends on T011)
- [x] T013 Run full `pnpm run check` (all frontend packages + `cargo check --workspace`) and `pnpm run format`; attempted live visual confirmation in the running dev app but the workspace browser tool cannot reach this sandbox's local dev server (`net::ERR_CONNECTION_REFUSED` even on the backend's own health endpoint, which curl from this shell reaches fine) — a pre-existing environment limitation, not something this change can fix. Rendered-DOM test coverage (T011) stands in for visual confirmation; documented in the final report (depends on T012)

<!--
Conventions:
- `T001` … task ids are stable and referenced by the dependency graph.
- `[P]` … parallel-safe (independent files). Omit for tasks that must be serial.
- `[ ]` / `[x]` … completion checkbox, toggled from the workbench.
-->
