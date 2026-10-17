# Tasks: Board search finds sub-issues

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files and may run in parallel within their group. Each task names the file(s) it changes.

## Phase 1: Setup
- [x] T001 Ensure worktree dependencies are installed (`pnpm install --frozen-lockfile` at repo root; changes no tracked files)

## Phase 2: Core
- [x] T002 Extract the memo body into an exported pure `filterKanbanIssues` and make `useKanbanFilters` call it, with no behaviour change, in `packages/web-core/src/features/kanban/model/hooks/useKanbanFilters.ts` (depends on T001)
- [x] T003 Apply the sub-issue exclusion only when the trimmed search query is empty (FR-1, FR-2), with a short comment citing why, in `packages/web-core/src/features/kanban/model/hooks/useKanbanFilters.ts` (depends on T002)

## Phase 3: Validation

The knowledge-base update, Codex review and PR are pipeline stages 11–13, not SpecKit tasks.

- [x] T004 Add Vitest coverage in `packages/web-core/src/features/kanban/model/hooks/useKanbanFilters.test.ts` (depends on T003):
  - no query: the sub-issue is hidden;
  - `190`, `swe-190` and a title substring each find it;
  - a whitespace-only query hides it;
  - the priority, assignee `self`, tag and hide-blocked filters each still exclude a matching sub-issue, and admit it when satisfied (analyze W1);
  - a match on `issue_number` alone, with an unrelated `simple_id` (analyze W2);
  - with `showSubIssues` on, nothing changes;
  - the input `filters` object is not mutated.
- [x] T005 Run the web-core Vitest suite, `pnpm run check`, `pnpm run lint` and `pnpm run format`, and fix any fallout in the files above (depends on T004)

<!--
Conventions:
- `T001` … task ids are stable and referenced by the dependency graph.
- `[P]` … parallel-safe (independent files). Omit for tasks that must be serial.
- `[ ]` / `[x]` … completion checkbox, toggled from the workbench.
-->
