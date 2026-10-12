# Feature Specification: Fix mobile issue list UI

**Feature dir**: `specs/vk/2b39-fix-mobile-ui/`
**Status**: Draft

## Summary
On a phone, the project issue list (Active / All / Backlog / Cancelled tabs) is
hard to read. Tag and relationship badges paint over the issue identifier, the
age column is clipped, and titles shrink to an ellipsis or vanish. The user
asked for mockups of possible fixes. This feature produces those mockups,
ships the recommended layout, and leaves desktop unchanged. (Constitution
LIII, XXXVI and IV.)

## User Stories
- As a user triaging on my phone, I want to read every issue's id, status and
  title at a glance, so that I can tell issues apart without opening each one.
- As a user on my phone, I want to still see tags, blockers and assignees, so
  that I do not lose the context the desktop list gives me.
- As the requester, I want to compare a few mocked-up layouts, so that I can
  see why the shipped one was chosen and ask for another if I prefer it.

## Functional Requirements
- FR-1: At mobile widths (the app's mobile layout, 390px reference), no part
  of a list row overlaps another part, and no row is wider than the list.
- FR-2: Each mobile row always shows the issue id, status, priority (when
  set), the age in full, and the title using all remaining width on its line.
- FR-3: Tags, relationship badges (with the existing "+N" overflow count) and
  assignees appear on a second line under the title, only when the issue has
  any. When that line is too narrow, it is cut off at the row edge and does
  not wrap into a third line.
- FR-4: Opening, drag-reordering and multi-select checkboxes behave as before
  on mobile.
- FR-5: The desktop list row looks and behaves exactly as today.
- FR-6: On mobile, the filter row (Team/Personal, search, filters, clear
  filters, new issue) fits on one line at 390px when filters are active. The
  clear-filters control becomes an icon with an accessible name.
- FR-7: Mockups of the current layout and at least three alternatives are
  checked into the feature dir, as an HTML source plus a rendered image, and
  the recommendation is stated.

## Out of Scope
- The kanban board/slim column views, the top navbar, the issue detail panel.
- Changing the number of tags/relationships shown or their data.

## Acceptance Criteria
- [ ] A 390px render of rows like the screenshot (2 tags+2 relationships, long
      titles) shows no overlap, and the age is not clipped.
- [ ] The desktop render of `IssueListRow` is unchanged (same markup branch).
- [ ] `pnpm run check`, `pnpm run lint` and `pnpm run format` pass.
- [ ] `mockups/mobile-issue-list.html` and `.png` exist and show options 0/A/B/C.

## Clarifications
See `clarifications.md`.
- Mockups only, or ship too? Both. The task pipeline ends in a merged PR, so
  the recommended option (A) ships and the mockups are kept in the feature
  dir. Switching to B or C later only touches `IssueListRow`'s mobile branch.
- Keep the metadata line or hide it on mobile? Keep it (option A). Hiding it
  (B) would drop blockers and tags, which the second user story says must
  stay visible.

## Open Questions
None remaining.
