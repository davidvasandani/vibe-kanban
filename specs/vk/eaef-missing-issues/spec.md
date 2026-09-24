# Feature Specification: Board search finds sub-issues

**Feature dir**: `specs/vk/eaef-missing-issues/`
**Status**: Clarified

## Summary
A user looking for an issue types its number into the project board's search box and gets zero results, even though the issue exists and opens normally. The reported case is SWE-190 on the Platform Ops board: an In progress, Urgent issue that is a sub-issue of SWE-176. The board's default Team view hides sub-issues, and that hide is applied before the search, so a sub-issue can never be found by searching that view. Nothing tells the user why. This feature makes an explicit search show every matching issue, sub-issues included, while the default board stays uncluttered (constitution XXXVIII).

## User Stories
- As a team member, I want searching the board for an issue's number or title to find it even when it is a sub-issue, so I can reach it without knowing which parent it belongs to.
- As a team member, I want the Team board without a search to stay as it is (top-level issues only), so the default view stays readable.
- As a team member who has also filtered by priority, assignee or tag, I want my search results to still respect those filters, so search does not undo choices I made on purpose.

## Functional Requirements
- FR-1: When the board search query is empty or whitespace-only, sub-issue visibility follows the view's "show sub-issues" setting exactly as it does today.
- FR-2: When the search query is non-empty, every issue matching it is eligible to appear, whether or not it is a sub-issue and regardless of the "show sub-issues" setting.
- FR-3: What counts as a match does not change: case-insensitive substring match on the title, the human-readable issue ID (e.g. `SWE-190`) or the issue number (e.g. `190`).
- FR-4: The priority, assignee (including the Personal view's "assigned to me"), tag and "hide blocked" filters still apply to search results.
- FR-5: A sub-issue shown because of a search is visibly marked as a sub-issue, as sub-issue cards already are.
- FR-6: Searching does not change the saved "show sub-issues" preference or any other saved view preference.
- FR-7: The behaviour is the same in every layout the search box filters, the kanban/slim column board and the list layout. All of them render the same filtered issue set.

## Out of Scope
- Changing which views hide sub-issues by default.
- The command-bar global search, which searches all issues separately and already finds sub-issues.
- Showing a "N sub-issues hidden" hint when no search is active. Deferred: the report concerns an issue the user explicitly searched for, and constitution III favours the smallest change. It can be raised as a follow-up.
- Server-side, deployment or other-service changes.

## Acceptance Criteria
- [ ] On a Team-view board with "show sub-issues" off, searching `190`, `swe-190`, or part of SWE-190's title shows SWE-190.
- [ ] With the same settings and an empty search, SWE-190 is not shown, as today.
- [ ] A search that matches a sub-issue while a priority or assignee filter excludes it does not show that sub-issue.
- [ ] With "show sub-issues" on, results are the same as before this change.
- [ ] Clearing the search restores the prior board contents, and the saved view preference is unchanged.
- [ ] Automated tests cover each criterion above; type checks, lint and formatting pass.

## Clarifications

### Session 2026-09-24
- Q: Does FR-7 include the list layout as well as the kanban columns? → A: Yes. `KanbanContainer` feeds the board and `IssueListView` from the same `filteredIssues`/`items`. The reported screenshot shows collapsible status groups, which matches the list layout.
- Q: Is a "sub-issues hidden" hint for the unfiltered board in scope? → A: No, it is deferred (see Out of Scope). The fix is limited to explicit search.

## Open Questions
- None remaining.
