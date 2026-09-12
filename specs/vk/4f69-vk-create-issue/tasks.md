# Tasks: Scrollable Create-Issue Settings

**Plan**: `./plan.md`

Tasks are dependency ordered. Tasks marked **[P]** touch independent files or
run independent verification lanes and may run together within their layer.

## Phase 1: Orientation

- [x] T001 Review the task inputs and confirm scope/constraints in `SPEC.md`,
      `../PRIOR_KNOWLEDGE.md`, `IMPLEMENTATION_PLAN.md`,
      `specs/vk/4f69-vk-create-issue/spec.md`, `clarifications.md`, `plan.md`,
      `research.md`, `data-model.md`, `contracts.md`, and
      `.specify/memory/constitution.md`.
- [x] T002 Inspect the existing shared component and rendered-component test in
      `packages/ui/src/components/KanbanIssuePanel.tsx` and
      `packages/remote-web/src/test/KanbanIssuePanel.test.tsx` (depends on T001).
- [x] T003 [P] Confirm the mobile/desktop panel hosts already establish the
      intended height/overflow boundary in
      `packages/web-core/src/pages/kanban/ProjectKanban.tsx` and
      `packages/web-core/src/pages/kanban/ProjectRightSidebarContainer.tsx`
      (depends on T001).

## Phase 2: Regression Coverage and Fix

- [x] T004 Add rendered-DOM regression coverage for the shell/body scroll
      contract and create-control containment in
      `packages/remote-web/src/test/KanbanIssuePanel.test.tsx` (depends on T002).
- [x] T005 Run the focused pre-fix regression test and confirm it fails for the
      missing shrink/scroll contract without changing files (depends on T004).
- [x] T006 Implement the minimal shared-panel layout correction and stable test
      selectors in `packages/ui/src/components/KanbanIssuePanel.tsx` (depends on
      T003, T005).
- [x] T007 Confirm the final diff preserves header placement, mode section
      ordering, form behavior, and service scope in
      `packages/ui/src/components/KanbanIssuePanel.tsx` and
      `packages/remote-web/src/test/KanbanIssuePanel.test.tsx` (depends on T006).

## Phase 3: Verification

- [x] T008 Install dependencies with `pnpm install --frozen-lockfile` if the
      repository's worktree-safe preflight indicates they are missing (depends
      on T007).
- [x] T009 [P] Run the focused remote-web `KanbanIssuePanel` Vitest file
      (depends on T008).
- [x] T010 [P] Run `@vibe/ui` and `@vibe/remote-web` TypeScript checks (depends
      on T008).
- [x] T011 Run the relevant frontend lint lane (depends on T009, T010).
- [x] T012 Run repository formatting with `pnpm run format` (depends on T011).
- [x] T013 Run `git diff --check` and inspect `git status --short` for unintended
      files (depends on T012).
- [x] T014 Record implementation and verification results in this task list and
      reconcile `SPEC.md`/`IMPLEMENTATION_PLAN.md` if implementation evidence
      changes the planned contract (depends on T013).

## Parallel Layers

- After T001, T002 and T003 are independent read-only orientation tasks.
- After implementation and dependency readiness, T009 and T010 exercise
  independent test/typecheck lanes and may run together.
- Formatting follows lint/type/test verification so final diff checks observe
  the formatted implementation.

## Implementation Results

- Pre-fix focused test: failed because the panel had no explicit tested scroll
  selectors and its body rendered `flex-1 overflow-y-auto` without `min-h-0`.
- Post-fix focused `KanbanIssuePanel` suite: 6 tests passed.
- `@vibe/ui` TypeScript check: passed.
- `@vibe/remote-web` TypeScript check: passed.
- `@vibe/ui` ESLint: passed.
- Root `pnpm run format`: passed after frozen dependency installation.
- `git diff --check`: passed; status contains only the task's root artifacts,
  feature directory, shared component, and focused test.
- Manual physical-device verification remains outside this headless worktree;
  automated coverage locks the browser-relevant flex/overflow contract.
