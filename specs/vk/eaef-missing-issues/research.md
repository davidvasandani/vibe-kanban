# Research: Board search finds sub-issues

## Root cause (verified)
- SWE-190 (`d692391d-…`) and its parent SWE-176 (`afc36cb5-…`) are both in project `29134795-…` (Platform Ops), and the MCP `get_issue` call returns both. So the data is intact and the issue is in the project's synced issue set.
- In the Team view, `getDefaultShowSubIssuesForView('team')` returns `false` (`packages/web-core/src/shared/stores/useUiPreferencesStore.ts`).
- `useKanbanFilters` removes every issue with a `parent_issue_id` before it runs the text search, so no query can match a sub-issue in that view. The screenshot matches this: all six groups show 0 for the query `190`.

## Decision: a non-empty search overrides the sub-issue hide
Alternatives considered:

1. **Show sub-issues by default in the Team view.** Rejected. It changes the uncluttered default for every user and is out of scope.
2. **Show a "N matching sub-issues hidden, show them" hint.** It still leaves the user one step from the result, and needs new UI and translations. It could be a follow-up; the smaller fix answers the report directly.
3. **Also include the parent of a matching sub-issue.** Rejected. It surfaces cards that do not match the query, and the sub-issue card already marks itself.
4. **Let search override every filter.** Rejected. Priority, assignee, tag and hide-blocked are deliberate user choices (constitution XXXVIII).

## No new dependencies
The change uses only the existing Vitest setup (`packages/web-core/vitest.config.ts`, node environment).
