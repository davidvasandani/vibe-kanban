# Kanban board filtering and sub-issue visibility

How the project board decides which issues it renders, and the ordering rule behind "the issue exists but the board search can't find it".

## One pipeline, one place

All board filtering runs in the pure `filterKanbanIssues` in `packages/web-core/src/features/kanban/model/hooks/useKanbanFilters.ts`. `useKanbanFilters` only memoises the assignee and tag lookup maps and calls it. Its output, `filteredIssues`, feeds the `items` rebuild in `KanbanContainer` (see `kanban-items-state-and-activity-grouping.md`). That one array drives the kanban/slim board and `IssueListView`, so a filter change reaches every layout and both frontends (`local-web` and `remote-web`).

The stages run in this order:
1. sub-issue hide;
2. text search (title, `simple_id`, `issue_number`);
3. priority;
4. assignee (`__self__` resolves to the current user, `unassigned` is special);
5. tags;
6. hide blocked (a blocker in a hidden status or the last visible column counts as resolved).

Sorting happens later, per column, in the container.

## Gotcha: view defaults versus explicit lookups

`getDefaultShowSubIssuesForView` hides sub-issues in the **Team** view and shows them in **Personal**. The hide used to run unconditionally before the search, so no search could match a sub-issue in the Team view. SWE-190, a sub-issue of SWE-176, returned zero results for `190` even though it existed and opened fine from a direct link.

The rule now, also constitution XXXVIII, is that the sub-issue hide applies only when the trimmed query is empty. A non-empty search reveals matching sub-issues, which keep their `isSubIssue` card marker. The deliberate filters (priority, assignee, tags, hide blocked) still apply, and searching never writes the `showSubIssues` preference.

If you add a new "curating" default, such as hiding a status group or scoping to a team, gate it the same way. Otherwise an explicit lookup silently fails. When an issue is reported "missing" from the board, first check whether a view default is removing it before the search runs, then check project membership and the Electric sync.

Rejected alternatives: changing the Team default (it clutters every board); a "N hidden" hint (it still costs a click and needs new UI and i18n); also revealing the parent of a match (it shows non-matching cards).

## Verification

`useKanbanFilters.test.ts` calls `filterKanbanIssues` with plain fixtures in the node Vitest environment; no React is needed. When a test asserts that something is absent (for example, hide blocked excluding a result), pair it with a positive case (the blocker resolved, so the result appears). A test that only expects `[]` also passes against the old code, so it proves nothing.

## Contributed by

- vk/eaef-missing-issues
