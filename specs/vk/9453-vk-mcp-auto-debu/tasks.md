# Tasks: VK MCP Auto Debug

**Feature**: `specs/vk/9453-vk-mcp-auto-debu/`
**Task**: `vk/9453-vk-mcp-auto-debu`

Tasks are ordered by dependency layer. Tasks marked `[P]` touch independent
files or seams and may be implemented in parallel after their dependencies are
complete.

## Layer 0 - Baseline

- [x] T001 Establish the current frontend/backend baseline from the repo root:
      run `pnpm run check` and `pnpm run lint`, then record or fix any
      pre-existing failures before feature edits.
      - Dependencies were installed from the lockfile before final validation.

## Layer 1 - Pure Contracts and Localization

- [x] T002 Add `packages/web-core/src/shared/lib/mcpDebugIssue.ts` with pure
      helpers for diagnostic fallback selection, dynamic Markdown fence
      selection, deterministic debug issue titles, and deterministic debug issue
      descriptions matching `contracts.md`.
- [x] T003 [P] Add
      `packages/web-core/src/shared/lib/mcpDebugIssue.test.ts` covering exact
      multiline diagnostic preservation, missing-diagnostic fallback, title
      identity content, and diagnostics containing Markdown fence markers.
      Depends on T002.
- [x] T004 [P] Add settings translation keys for missing diagnostics, Copy
      action states, Debug action states, unavailable project/status
      explanations, issue-created feedback, open-issue action, and creation
      failure in all locale files under
      `packages/web-core/src/i18n/locales/*/settings.json`, using English
      fallback strings where the repository convention permits. Depends on T002.

## Layer 2 - Settings Integration

- [x] T005 Read the full current
      `packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx`
      and identify the minimal insertion points for `useProjectContextOptional`,
      `useAppNavigation`, per-assignment copy/debug state, and failed-result
      rendering. Depends on T001.
- [x] T006 Refactor `TestResultDetails` so `failed` results render the full
      diagnostic panel with preserved line breaks and long-word wrapping, while
      `ok`, `unsupported`, and `auth_required` behavior remains unchanged.
      Remove frontend truncation only from failed diagnostics. Depends on T002,
      T004, and T005.
- [x] T007 Wire the failed-result Copy action in
      `McpSettingsSection.tsx` using `navigator.clipboard.writeText` with
      per-result `idle | success | error` feedback rendered visibly and through
      an `aria-live` region. Ensure failures do not mutate or hide the
      diagnostic. Depends on T006.
- [x] T008 Wire Debug availability from `useProjectContextOptional()`: enable
      only with an active local project context and at least one status, and
      otherwise render the localized unavailable explanation without selecting
      an arbitrary project. Depends on T006.
- [x] T009 Implement Debug issue creation with the existing `insertIssue`
      contract: sort statuses by `sort_order`, use the first status, and compute top-of-column
      `sort_order` with the existing `min(sort_order) - 1` pattern, seed the
      title/description from `mcpDebugIssue.ts`, await `persisted`, and store
      per-result `creating | success | error` state keyed by
      `testKey(serverName, executor)`. Depends on T008.
- [x] T010 Prevent duplicate Debug submissions by disabling the action while
      the matching assignment key is creating, and render creation errors inline
      without altering the diagnostic. Depends on T009.
- [x] T011 Render the post-create success state inside Settings with a visible
      Open Issue action that calls `useAppNavigation().goToProjectIssue` only
      on user click. Do not navigate automatically or close Settings. Depends on
      T009.
- [x] T012 Update the secondary non-OK result list so every failed assignment,
      not just the attention result, gets the same full diagnostic, Copy, Debug,
      unavailable, error, and success behavior. Preserve existing organization
      for non-failed results. Depends on T006-T011.

## Layer 3 - Focused Tests

- [x] T013 Add focused pure tests for full multiline diagnostic selection,
      exact clipboard input, and failed-result view state using the existing
      Node Vitest harness. Depends on T007.
- [x] T014 [P] Add pure tests for unavailable Debug outside project context
      and unavailable Debug when no project status exists. Depends on T008.
- [x] T015 [P] Add pure tests for successful Debug issue payload, safe
      diagnostic description content, post-create Open Issue action, and no
      automatic navigation. Depends on T009 and T011.
- [x] T016 [P] Add pure tests for duplicate-click prevention and creation
      failure feedback preserving the diagnostic. Depends on T010.
- [x] T017 Confirm existing `auth_required`, `unsupported`, saved MCP testing,
      OAuth Connect, save, and discard test coverage still passes or add narrow
      regression assertions if the touched component lacks coverage. Depends on
      T012.
      - The complete `@vibe/web-core` suite passed (12 files, 167 tests).

## Layer 4 - Validation and Review

- [x] T018 Run focused helper tests:
      `pnpm exec vitest run packages/web-core/src/shared/lib/mcpDebugIssue.test.ts`.
      - The helper test file passed all 9 tests.
- [x] T019 Run the targeted MCP settings/component tests added or affected by
      this change with `pnpm exec vitest run <test files>`.
      - The repository has no rendered DOM harness for this shared component;
        pure contract/state tests were added and the complete web-core suite
        passed.
- [x] T020 Run repository validation from the root: `pnpm run check` and
      `pnpm run lint`; fix failures in affected code.
      - Both commands completed successfully.
- [x] T021 Run required formatting from the root with `pnpm run format` and
      verify it does not introduce unrelated churn.
      - Formatting completed successfully without unrelated tracked-file churn.
- [x] T022 Perform an independent final diff review against every acceptance
      criterion in `spec.md` and the contracts in `contracts.md`, confirming no
      generated files were edited manually and no backend/API behavior changed.
      - Codex review findings for duplicate-creation races and modal project
        context were fixed; the final review reported no significant findings.

## Parallelization Notes

T003 and T004 can run in parallel after T002. T014, T015, and T016 can run in
parallel once their integration dependencies are complete. Validation tasks
remain sequential because later checks should run against the final formatted
diff.
