# Tasks: Lint web-core and remote-web

Plan: `specs/vk/848f-lint-packages-we/plan.md`. `[P]` = touches files no other
task in the same group touches; safe to do together.

## Group A — config and wiring
- [x] T001 Extract local-web rules into `eslint.frontend.cjs`; reduce
      `packages/local-web/.eslintrc.cjs` to a factory call; prove
      `--print-config` unchanged.
- [x] T002 [P] `packages/web-core/.eslintrc.cjs`, `packages/web-core/tsconfig.eslint.json`,
      `lint` script in `packages/web-core/package.json`.
- [x] T003 [P] `packages/remote-web/.eslintrc.cjs`, `packages/remote-web/tsconfig.eslint.json`,
      `lint` script + ESLint devDependencies in `packages/remote-web/package.json`,
      `pnpm-lock.yaml`.
- [x] T004 [P] Root `package.json`: `web-core:lint`, `remote-web:lint`, add both to `lint`.
- [x] T005 [P] `.github/workflows/test.yml`: `core:lint`, `remote:lint` in `frontend-checks`.
- [x] T006 Naming rule in `eslint.frontend.cjs`: PascalCase for `src/**/!(use*).tsx`,
      camelCase for `src/**/use*.{ts,tsx}`.

## Group B — moves and renames (depends on A)
- [x] T007 [P] `createModeSeedStore.ts` → `web-core/src/shared/stores/`; update
      `shared/actions/index.ts`, `pages/workspaces/WorkspacesLayout.tsx`, and
      intra-feature importers.
- [x] T008 [P] `CreateChatBoxContainer.tsx`, `CreateModeRepoPickerBar.tsx` →
      `web-core/src/features/create-mode/ui/`; update
      `pages/kanban/ProjectRightSidebarContainer.tsx`,
      `pages/workspaces/WorkspacesLayout.tsx`,
      `remote-web/src/test/CreateModeRepoPickerBar.test.tsx`.
- [x] T009 [P] `SharedAppLayout.tsx` → `web-core/src/pages/root/`; update
      `local-web/src/routes/_app.tsx` and its `./` sibling imports.
- [x] T010 [P] Relative intra-feature imports in
      `features/create-mode/model/CreateModeProvider.tsx`, `useCreateModeState.ts`.
- [x] T011 [P] Rename `shared/dialogs/settings/settings/settingsRegistry.tsx` →
      `SettingsRegistry.tsx`; update `SettingsDialog.tsx`, `SettingsSection.tsx`.

## Group C — code findings (depends on B where files overlap)
- [x] T012 [P] Unused vars: `web-core/src/shared/lib/localApiTransport.ts`,
      `remote-web/src/shared/lib/relayHostApi.ts`,
      `web-core/src/shared/stores/useKanbanIssueComposerStore.ts`,
      `web-core/src/pages/workspaces/WorkspacesMainContainer.tsx`,
      `web-core/src/shared/hooks/useIssueMultiSelect.ts`,
      `remote-web/src/shared/lib/webrtc/dataChannelWebSocket.ts`.
- [x] T013 [P] `web-core/src/shared/hooks/useLocalStorageScratch.ts` (no-empty),
      `web-core/src/shared/components/MarkdownPreview.tsx` (directives, typing).
- [x] T014 [P] `remote-web/src/shared/lib/webrtc/connection.ts` (switch),
      delete `remote-web/src/shared/lib/webrtc/index.ts`, update `app/entry/Bootstrap.tsx`.

## Group D — exhaustive-deps (one judgement each; see plan §7)
- [x] T015 [P] `features/workspace-chat/model/hooks/useConversationHistory.ts`
- [x] T016 [P] `features/workspace-chat/model/useConversationVirtualizer.ts`
- [x] T017 [P] `features/workspace-chat/ui/ConversationListContainer.tsx`
- [x] T018 [P] `features/workspace-chat/ui/SessionChatBoxContainer.tsx`,
      `pages/workspaces/ChangesPanelContainer.tsx`, `shared/hooks/useAzureAttachments.ts`
- [x] T019 [P] `features/create-mode/ui/CreateModeRepoPickerBar.tsx` (after T008)
- [x] T020 [P] `shared/dialogs/command-bar/CreatePRDialog.tsx`,
      `StartReviewDialog.tsx`, `shared/dialogs/tasks/ResolveConflictsDialog.tsx`,
      `shared/dialogs/kanban/AssigneeSelectionDialog.tsx`
- [x] T021 [P] `shared/dialogs/settings/settings/McpSettingsSection.tsx`,
      `PipelinesSettingsSection.tsx`, `ReposSettingsSection.tsx`, `McpServerDialog.tsx`
- [x] T022 [P] `shared/hooks/useRemoteCloudHosts.ts`, `shared/providers/ActionsProvider.tsx`
- [x] T023 [P] remote-web: `app/layout/RemoteAppShell.tsx`,
      `app/layout/RemoteNavbarContainer.tsx`, `app/providers/RemoteActionsProvider.tsx`

## Group E — verification and docs
- [x] T024 `pnpm run lint` (incl. both new lints) exits 0; negative proof with an
      unused import in each package.
- [x] T025 [P] `pnpm run check`; web-core and remote-web vitest suites.
- [x] T026 [P] `pnpm run format`.
- [x] T027 Update `wiki/workspace-sidebar-filtering.md` "Verification notes".
- [x] T028 [P] `AGENTS.md`: the lint line lists web-core and remote-web.
- [x] T029 [P] `wiki/external-connector-sync.md`,
      `docs/knowledge-base/remote-external-integrations.md`: `SettingsRegistry.tsx`.
