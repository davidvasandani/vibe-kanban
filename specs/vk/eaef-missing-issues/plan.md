# Implementation Plan: Board search finds sub-issues

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context
- The change is frontend-only, in `packages/web-core` (React 18, TypeScript, zustand UI preferences). Vitest runs in the node environment (`packages/web-core/vitest.config.ts`).
- Board data is the project's Electric-synced `issues` collection, filtered client-side. No server, schema, generated types or persisted data change.
- The web-core change reaches both `local-web` and `remote-web` (constitution IV).

## Architecture & Approach
All sub-issue hiding for the board happens in one place:
`packages/web-core/src/features/kanban/model/hooks/useKanbanFilters.ts`.

```ts
if (!showSubIssues) {
  result = result.filter((issue) => issue.parent_issue_id === null);
}
// ...then text search, priority, assignee, tags, hide-blocked
```

`KanbanContainer.tsx` passes the hook's `filteredIssues` into the `items` rebuild effect. The kanban/slim board and `IssueListView` both render from that array, which satisfies FR-7.

Changes:

1. **Extract a pure function.** Add an exported `filterKanbanIssues(params)` in the same module, holding the current memo body. It takes `issues`, the `assigneesByIssue` / `tagsByIssue` maps, `issueRelationships`, `issuesById`, `doneStatusIds`, `filters`, `showSubIssues`, `hideBlocked` and `currentUserId`. `useKanbanFilters` keeps its public signature and its memoised lookup maps, and its `filteredIssues` memo calls `filterKanbanIssues`. This mirrors `features/kanban/model/activityGrouping.ts`: pure logic plus colocated Vitest.
2. **Gate the sub-issue hide on the query (FR-1, FR-2).** Compute `query = filters.searchQuery.trim().toLowerCase()` first, and apply the `parent_issue_id === null` exclusion only when `!showSubIssues && !query`. The matcher stays as it is (FR-3), and the priority, assignee, tag and hide-blocked filters run afterwards on the result (FR-4).
3. **Leave everything else alone.** Cards already get `isSubIssue={!!issue.parent_issue_id}` (FR-5, `KanbanContainer.tsx`). Search writes only `filters.searchQuery`, so the `showSubIssues` preference is untouched (FR-6).
4. **Tests.** Add `packages/web-core/src/features/kanban/model/hooks/useKanbanFilters.test.ts`, calling `filterKanbanIssues` with fixture issues (a top-level parent and the SWE-190-like sub-issue). One test per acceptance criterion; see `./tasks.md`.

## Data Model
None. `Issue.parent_issue_id`, `simple_id` and `issue_number` from `shared/remote-types.ts` are read, never changed.

## Contracts
None beyond the new exported function signature described above, which is internal to web-core.

## Research Notes
See `./research.md`.

## Constitution Check
- **I. Clarity:** the change reorders one conditional and adds a short comment explaining why search overrides the hide.
- **II. Test the contract:** Vitest covers every acceptance criterion, including the negative case where the sub-issue stays hidden without a query.
- **III/VI. Small, reuse:** the existing matcher and filter pipeline are reused, with no new state, preference or UI.
- **IV. Blast radius:** both frontends consume web-core's board, and the behaviour is intended for both.
- **XXXIV/XXXV:** not affected; no enrichment or identity projection is involved.
- **XXXVIII. Explicit lookups:** this is the principle being implemented. A search reveals hidden-by-default sub-issues, deliberate filters still apply, the marker is kept and the preference is not mutated.
- **Constraints:** no new dependencies, no generated-file edits, and `pnpm run format` runs before completion.

No deviations.

## Risks & Dependencies
- **Drag-and-drop while searching:** a sub-issue revealed by search can be dragged, which persists `sort_order` for the visible column. Dragging a top-level issue while a search is active already behaves this way, so it is not a new class of risk.
- **Performance:** unchanged. The query just skips one filter pass.
- **Verification** depends on `pnpm install --frozen-lockfile` being present in the worktree.
