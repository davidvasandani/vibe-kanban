# Feature Specification: Filter sidebar workspaces by issue status

**Feature dir**: `specs/vk/4dac-filter-workspace/`
**Status**: Draft

## Summary
The workspaces sidebar lists every workspace. Many are linked to an issue that
has moved past the point where the operator needs to watch it, for example an
issue in **In review** that is waiting on a human reviewer. Today the filter
dialog can narrow the list only by project and by PR state, so these
workspaces crowd out the ones that need attention. This feature lets the
operator switch specific issue statuses off, and hides every workspace whose
linked issue is in one of those statuses.

## User Stories
- As an operator, I want to hide workspaces whose issue is "In review" so that
  the sidebar shows only work that still needs me.
- As an operator working across several projects, I want hiding a status to
  apply in every project that has a status with that name, so that I don't
  have to repeat the choice per project.
- As an operator, I want my choice to survive a reload so that I set it once.
- As an operator, I want to see at a glance that a filter is hiding
  workspaces, and clear it in one action, so that I never lose track of work.

## Functional Requirements
- FR-1: The sidebar filter dialog offers an issue-status control that lists
  the statuses of the projects the operator's workspaces belong to. Each
  status name appears once, in board order.
- FR-2: Each listed status can be toggled hidden or shown. A workspace whose
  linked issue is in a hidden status is removed from the sidebar list.
- FR-3: Status matching is by name and ignores letter case and surrounding
  whitespace, so "In review" hides "In Review" in every project.
- FR-4: What is stored is the set of *hidden* statuses. A status that the
  operator has never seen (a new column, a new project) is shown by default.
- FR-5: A workspace is hidden only when its linked issue's status is known and
  hidden. A workspace whose issue or status information is still loading or
  unavailable stays visible.
- FR-6: The control also offers a "No issue" option, which hides workspaces
  that are not linked to any issue. Unselected, it leaves them visible.
- FR-7: A status the operator hid stays listed, so it can be un-hidden, even
  when no current project has it.
- FR-8: The status filter combines with the project filter, the PR filter and
  the search box. A workspace must pass all of them. It applies to both the
  active and the archived list.
- FR-9: The filter icon shows its active state while any status is hidden.
  "Clear filters" also clears the hidden statuses.
- FR-10: The choice persists across reloads and devices, with the other
  sidebar filter preferences. Preferences saved before this feature still
  load, with nothing hidden.
- FR-11: The default sidebar, with no status hidden and the dialog closed, does
  no extra data loading for this feature.

## Out of Scope
- Filtering the carousel view or the kanban board.
- Choosing different statuses per project.
- Showing a status badge on each sidebar row.
- A "show only" (allow-list) mode.

## Acceptance Criteria
- [ ] Hiding "In review" removes every workspace whose linked issue is In
      review, in any project, from the sidebar. Un-hiding restores them.
- [ ] Workspaces with no linked issue stay visible unless "No issue" is
      selected.
- [ ] A workspace whose issue has not loaded is visible.
- [ ] Hidden statuses survive a reload.
- [ ] The filter icon is highlighted while any status is hidden, and "Clear
      filters" restores the full list.
- [ ] Automated tests cover case-insensitive cross-project matching,
      fail-open, "No issue", composition with the other filters, and building
      the option list.

## Clarifications

Resolved in `/speckit.clarify`. No answers from the operator were supplied, so
each default matches how the existing project and PR filters already behave.

- **Group headers / "N hidden" hint.** The status filter behaves exactly like
  the project and PR filters. Accordion groups list only the workspaces that
  pass every filter. The flat view's "Active" header keeps showing the
  unfiltered active total, as it does today. No separate "N hidden" hint is
  added, because the highlighted filter icon (FR-9) already shows that a
  filter is on. → FR-12.
- **Attention bypass.** A workspace that needs attention (a pending approval,
  or unseen output) does **not** bypass the status filter. The operator chose
  to hide that status on purpose, and the existing filters make no such
  exception (constitution XXXVIII: deliberate filters still apply). → FR-13.

- FR-12: Sidebar group contents and counts treat status-hidden workspaces the
  same way they already treat workspaces removed by the project or PR filter.
- FR-13: The status filter applies to every workspace, including workspaces
  that need attention.

## Open Questions
None remaining.
