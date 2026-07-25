# Tasks: Settings Pipelines Editor

**Feature**: `specs/vk/3a97-no-frontend-for/`  
**Task**: `3a97-no-frontend-for`

`[P]` = parallelizable within its dependency layer. Parallel tasks either touch
different files, add independent tests/locales, or run read-only verification.
Tasks that edit the same file or depend on a behavior contract are intentionally
sequential.

## Layer 1 - Contracts, Helpers, And Test Seams

- [x] T001 Extend `MachineClient` in
      `packages/web-core/src/shared/lib/machineClient.ts` with host-routed
      pipeline methods for status list, raw read, validation, raw write,
      reset-one, reset-all-defaults, and delete, using `makeMachineRequest`,
      `encodeURIComponent`, and generated types from `shared/types.ts`.
- [x] T002 Add pure pipeline Settings helpers in
      `packages/web-core/src/shared/lib/pipeline/pipelineSettings.ts` for slug
      validation, bundled-id detection, starter TOML generation, parse-error
      location formatting, validation tuple comparison, and post-refresh
      selection.
- [x] T003 [P] Add focused Vitest coverage in
      `packages/web-core/src/shared/lib/pipeline/pipelineSettings.test.ts` for
      valid and invalid ids, bundled ids, 1-based location formatting with
      missing line/column cases, raw starter template output, tuple matching,
      and keep-current-or-first selection behavior.
- [x] T004 Add Settings pipeline query keys and hooks in
      `packages/web-core/src/shared/hooks/usePipelines.ts`: status query keyed by
      `['pipeline-statuses', ...machineClient.queryScopeKey]`, raw query keyed
      by `['pipeline-raw', ...machineClient.queryScopeKey, pipelineId]`,
      disabled scope `['machine', 'unselected']`, validation mutation, write,
      reset-one, reset-all, and delete mutations.
- [x] T005 Add or extend hook/query-key tests in
      `packages/web-core/src/shared/hooks/usePipelines.test.ts` or a nearby
      existing hook test file to cover host-aware status/raw keys, disabled
      unselected keys, and mutation invalidation of same-scope status/raw keys
      plus legacy `PIPELINES_QUERY_KEY`. Depends on T004.

## Layer 2 - Settings Navigation And Dirty Host Switching

- [x] T006 Update dirty-state host switching in
      `packages/web-core/src/shared/dialogs/settings/SettingsDialog.tsx` so
      Settings host changes ask for confirmation when any section is dirty,
      cancelled switches preserve the current host and draft, and confirmed
      switches clear the discarded dirty state before mounted sections reseed
      from the new `MachineClient`.
- [x] T007 Add focused component coverage in
      `packages/web-core/src/shared/dialogs/settings/SettingsDialog.test.tsx` or
      an existing Settings dialog test file for dirty host-switch confirmation,
      cancelled host switch preservation, and confirmed host switch reseeding.
      Depends on T006.

## Layer 3 - Pipelines Settings Section

- [x] T008 Create
      `packages/web-core/src/shared/dialogs/settings/settings/PipelinesSettingsSection.tsx`
      with host-scoped status loading through `useSettingsMachineClient()`,
      explicit loading/empty/error states, pipeline id keyed selection, valid and
      invalid rows, stage count display, and invalid status errors with optional
      1-based line/column.
- [x] T009 Implement existing-file raw TOML loading in
      `PipelinesSettingsSection.tsx`, seeding `draftContent`,
      `lastPersistedContent`, selected id, draft kind, validation state, and
      mutation error from the current machine scope so modal reopen or host
      switch cannot leak stale state. Depends on T008.
- [x] T010 Implement the monospace multiline raw TOML editor, dirty flag
      registration through `SettingsDirtyContext`, and unchanged-content Save
      disabling in `PipelinesSettingsSection.tsx`. Depends on T009.
- [x] T011 Implement debounced draft validation and explicit save validation in
      `PipelinesSettingsSection.tsx`, always sending the selected existing id or
      proposed new id, applying results only for the latest host/id/content
      tuple, and showing pending, valid, and invalid feedback. Depends on T002,
      T004, T010.
- [x] T012 Implement Save in `PipelinesSettingsSection.tsx` so only validated
      changed TOML is written unchanged, Save remains disabled while validation
      or write is pending, mutation failures remain visible without discarding
      drafts, and successful saves keep the saved id selected. Depends on T011.
- [x] T013 Implement Add flow in `PipelinesSettingsSection.tsx`: require a valid
      ASCII alphanumeric/underscore/hyphen id, reject exact conflicts with
      existing status ids for the selected host, open the clarified one-stage
      draft template, and create no host file until Save. Depends on T002, T012.
- [x] T014 Implement unsaved-draft guards in
      `PipelinesSettingsSection.tsx` for file selection, starting Add, closing
      a new-draft flow, deleting/resetting the selected file, reset-all when the
      open draft would be overwritten, and host reseed after confirmed host
      switch. Depends on T006, T013.
- [x] T015 Implement delete in `PipelinesSettingsSection.tsx` for existing files
      only, with confirmation copy that mentions final-file default reseeding,
      status refresh, and next-file selection after deleting the selected file.
      Depends on T014.
- [x] T016 Implement bundled reset-one and reset-all-defaults actions in
      `PipelinesSettingsSection.tsx`, limiting per-file reset to `basic`,
      `wikillm`, `speckit`, and `parallel-subagents`, requiring overwrite
      confirmation, explaining draft discard when reset-all would overwrite the
      open dirty draft, preserving unrelated dirty drafts, and refreshing editor
      and list state after success. Depends on T014.
- [x] T017 Ensure every successful create/save/delete/reset mutation invalidates
      host-scoped status and raw queries for the selected
      `MachineClient.queryScopeKey`, invalidates legacy `PIPELINES_QUERY_KEY`,
      and invalidates any host-aware task-create pipeline key if implementation
      introduces one. Depends on T012, T015, T016.

## Layer 4 - Registration, Localization, And UI Shape

- [x] T018 Register the host-scoped Pipelines section in
      `packages/web-core/src/shared/dialogs/settings/settings/settingsRegistry.tsx`
      with existing Settings navigation conventions and an appropriate existing
      icon. Depends on T008.
- [x] T019 [P] Add English `settings` namespace strings for navigation, labels,
      statuses, validation states, confirmations, errors, empty states, delete
      reseeding explanation, reset actions, and mutation feedback in
      `packages/web-core/src/i18n/locales/en/settings.json`.
- [x] T020 [P] Mirror the full new locale key structure with repository
      fallback strings in
      `packages/web-core/src/i18n/locales/es/settings.json`,
      `packages/web-core/src/i18n/locales/fr/settings.json`,
      `packages/web-core/src/i18n/locales/ja/settings.json`,
      `packages/web-core/src/i18n/locales/ko/settings.json`,
      `packages/web-core/src/i18n/locales/zh-Hans/settings.json`, and
      `packages/web-core/src/i18n/locales/zh-Hant/settings.json`. Depends on
      T019.
- [x] T021 Review responsive Settings layout in
      `PipelinesSettingsSection.tsx` for normal and narrow dialog sizes: no
      nested cards, explicit pending/error states, stable file-list/editor
      sizing, monospace textarea readability, and no hidden raw TOML rewrites.
      Depends on T018, T020.

## Layer 5 - Focused Frontend Verification

- [x] T022 Add component or integration tests in
      `packages/web-core/src/shared/dialogs/settings/settings/PipelinesSettingsSection.test.tsx`
      for host A/B status loading, malformed file display with message and
      location, existing raw content loading without reformatting, invalid draft
      disabling Save, valid changed draft enabling Save, and stale validation
      responses not changing visible state. Depends on T011, T021.
- [x] T023 Extend `PipelinesSettingsSection.test.tsx` coverage for Add conflict
      rejection before draft open, unsaved file-switch confirmation, mutation
      failure preserving draft content, delete selected file selecting another
      status, reset-one confirmation, and reset-all refresh behavior. Depends on
      T015, T016, T022.
- [ ] T024 [P] Run focused frontend tests for pipeline helper, hook/query-key,
      Settings dirty host switching, and Pipelines Settings section behavior.
      Depends on T003, T005, T007, T023.
- [ ] T025 [P] Run the web-core TypeScript check and resolve only issues caused
      by this feature. Depends on T021.
- [x] T026 [P] Run localization shape verification, or add a lightweight script
      check if none exists, proving all seven
      `packages/web-core/src/i18n/locales/*/settings.json` files contain the
      same new Pipelines key structure. Depends on T019, T020.

## Layer 6 - Repository Verification And Closure

- [ ] T027 Run `pnpm run check` to verify frontend and backend workspace checks
      after the completed frontend integration. Depends on T024, T025, T026.
- [ ] T028 Run `pnpm run lint` and fix only issues introduced by the Pipelines
      Settings feature. Depends on T027.
- [ ] T029 Run `pnpm run format` before completion, as required by `AGENTS.md`,
      then inspect the diff to ensure formatting did not rewrite unrelated user
      changes. Depends on T028.
- [ ] T030 Manually smoke-test the Settings UI when the local dev environment is
      available: host A versus host B status loading, invalid row with
      line/column, raw TOML preservation, invalid/valid validation states,
      create deferred until Save, save refresh, delete, reset-one, reset-all,
      mutation failure visibility, dirty file switching, and dirty host
      switching. Depends on T029.
- [x] T031 Review the complete diff against
      `specs/vk/3a97-no-frontend-for/spec.md` acceptance criteria, the
      constitution's Settings host-scope boundary, and the frontend-only
      constraint; confirm no backend route or generated `shared/types.ts` edits
      were introduced. Depends on T030.
- [ ] T032 Update project knowledge after shipping, if implementation confirms
      reusable file-backed Settings editor conventions, in
      `docs/knowledge-base/` and its index with task id `3a97-no-frontend-for`.
      Depends on T031.

## Suggested Commands

- `pnpm --filter @vibe/web-core run test`
- `pnpm run check`
- `pnpm run lint`
- `pnpm run format`

## Dependency And Parallelization Notes

- Layers are dependency ordered: API/helpers -> host-switch dirty guard ->
  section implementation -> registration/locales -> tests -> verification.
- T003 can run after helper signatures are sketched and does not need UI work.
- T019 and T020 are independent of TypeScript behavior once the intended key
  names are known, but T020 depends on T019 for the canonical English key tree.
- T024, T025, and T026 are parallel read-only verification commands after their
  implementation prerequisites land.
- `shared/types.ts` is generated output and should not be edited for this
  feature because no backend response shape changes are in scope.
