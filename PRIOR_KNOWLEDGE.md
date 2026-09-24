# Prior knowledge for "missing issues" (sub-issues invisible to board search)

Distilled from both knowledge bases, `vibe-kanban/wiki/` and `vibe-kanban/docs/knowledge-base/`. This was a read-only pass; nothing in either base changed. Neither has a page on board filtering or on sub-issue visibility. Two pages touch the task.

## 1. Kanban `items` state (`wiki/kanban-items-state-and-activity-grouping.md`)

- `KanbanContainer` rebuilds `items: Record<statusId, issueId[]>` from `filteredIssues` inside an effect. Board render order, drag-and-drop indexes and the persisted `sort_order` all depend on that array.
- Implication: the fix belongs in `useKanbanFilters` (the input to `filteredIssues`), not at render time. Changing which issues enter `filteredIssues` is safe. The rebuild effect already reacts to that input, and dragging a search-revealed sub-issue persists `sort_order` the same way as any other card in the column.
- `items` feeds both the board and `IssueListView`, so the fix applies to both views. That is intended.

## 2. Global search (`wiki/global-search.md`)

- The command-bar global search queries the remote issues table by `simple_id`, so it already finds sub-issues. The board search is a separate client-side filter over the project's Electric-synced issues. This task changes only the board filter.
- The page's rule that `simple_id` is the human-facing identifier matches the board matcher, which already checks `simple_id` and `issue_number`.

## Not found

- Nothing records why the Team view hides sub-issues by default (`getDefaultShowSubIssuesForView`). Treat that default as deliberate and keep it; widen visibility only while a search query is active.
- No page covers Vitest conventions for web-core hooks. The nearest precedent is the pure-helper test `features/kanban/model/activityGrouping.test.ts`.
