# Implementation plan: board search finds sub-issues

1. **Extract a pure filter.** In `packages/web-core/src/features/kanban/model/hooks/useKanbanFilters.ts`, move the body of the `filteredIssues` memo into an exported pure function `filterKanbanIssues(params)`. It takes the same inputs plus the precomputed `assigneesByIssue` / `tagsByIssue` maps. The hook keeps its signature and memoisation and calls the function. This follows the `activityGrouping.ts` precedent, where model logic lives in pure functions tested with Vitest, without React.
2. **Reorder so search overrides the sub-issue hide.** Compute the trimmed, lower-cased query first. Apply the `showSubIssues === false` exclusion only when the query is empty. Leave matching (title / `simple_id` / `issue_number`) and every later filter (priority, assignee, tags, hide blocked) unchanged.
3. **Tests.** Add `useKanbanFilters.test.ts` next to the hook, covering:
   - no query, `showSubIssues` false: the sub-issue is hidden;
   - `showSubIssues` false: the sub-issue is found by simple ID (`190`, `swe-190`), by issue number and by title;
   - a whitespace-only query behaves like an empty one;
   - query plus a priority filter: a matching sub-issue with the wrong priority is excluded;
   - query plus the assignee `self` filter behaves the same way;
   - `showSubIssues` true: unchanged behaviour.
4. **Verify.** Run `pnpm install --frozen-lockfile` if `node_modules` is missing, then the web-core Vitest suite, `pnpm run check` (the frontend portion at minimum), `pnpm run lint` and `pnpm run format`.
5. **Review, knowledge and PR** (stages 11–13): run a Codex review of the diff, add a knowledge page on board filter ordering, then commit, open a PR against `main` and merge it.

Risks: none to persisted data, because the change is client-side filtering only. The Team board gains cards only while a query is typed.
