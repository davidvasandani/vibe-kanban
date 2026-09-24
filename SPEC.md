# Searching the kanban board must find sub-issues

## Problem

SWE-190 ("Add device-status and MX uplink-status collectors…", In progress, Urgent) exists in the Platform Ops project and opens fine in the issue panel, but typing `190` into the board search in the **Team** view returns zero matches in every column.

SWE-190 is a sub-issue of SWE-176. The Team view hides sub-issues by default (`getDefaultShowSubIssuesForView('team') === false`), and `useKanbanFilters` applies that hide **before** the text search. A sub-issue can therefore never be found by searching the Team board — not by title, simple ID or issue number — even though the search box is the tool a user reaches for when an issue "is missing". Nothing on screen says results were hidden because they are sub-issues.

## Requirement

When the board search query is non-empty, issues that match the query must be shown whether or not they are sub-issues. The "show sub-issues" preference governs only the unfiltered board.

- Empty or whitespace-only query: behaviour unchanged — sub-issues are hidden when `showSubIssues` is false.
- Non-empty query: matching sub-issues are included. Matching is unchanged (case-insensitive substring on title, `simple_id` and `issue_number`).
- Every other filter still applies to search results: priority, assignee (including Personal-view "self"), tags and "hide blocked". Search widens only the sub-issue exclusion, nothing else.
- Sub-issue cards found by search keep their existing sub-issue indicator (`isSubIssue`), so the user can see why the card is not normally on the board.
- The saved `showSubIssues` preference and the "active filters" indicator are not changed by searching.

## Out of scope

- Changing the Team view's default for `showSubIssues`.
- Server-side or global search (`wiki/global-search.md` covers that separately; it already searches all issues).
- Any deployment or homelab changes.

## Acceptance

- Unit tests for `useKanbanFilters` cover: a sub-issue hidden with no query; a sub-issue found by simple ID, issue number and title when `showSubIssues` is false; a search that also has a priority/assignee filter still excluding a non-matching sub-issue; top-level results unchanged.
- `pnpm run check`, `pnpm run lint` and `pnpm run format` pass; the web-core Vitest suite passes.
- Independent Codex review with no significant findings; knowledge base updated; PR merged to the base branch.
