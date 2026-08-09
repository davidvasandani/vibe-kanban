# Feature Specification: Scrollable Create-Issue Settings

**Feature dir**: `specs/vk/4f69-vk-create-issue/`  
**Status**: Draft

## Summary

Make the issue creation panel usable on constrained-height screens by ensuring
all settings and the final create action remain reachable through vertical
scrolling. This fixes the reported mobile experience where the lower controls
are cut off at the viewport boundary and cannot be scrolled into view.

## User Stories

- As a user creating an issue on a mobile or short screen, I want to scroll
  through every create setting so that I can review and configure the issue.
- As a user creating an issue, I want the Create Issue action to remain
  reachable regardless of the visible screen height so that I can finish the
  workflow.
- As a user viewing or editing an existing issue, I want the panel's current
  navigation and section behavior to remain unchanged.

## Functional Requirements

- **FR-1:** The issue panel must provide vertical scrolling whenever its content
  exceeds the panel's available height.
- **FR-2:** In create mode, every setting rendered below the description,
  including optional pipeline controls and the draft-workspace setting, must be
  inside the reachable scrolling content.
- **FR-3:** The Create Issue action and draft-delete action, when present, must
  be reachable through the same scrolling content.
- **FR-4:** The issue identity/close header must remain outside the scrolling
  content and visible while the body is scrolled.
- **FR-5:** The scrolling behavior must apply consistently to the shared panel
  used by both local and remote frontends and to both create and edit modes.
- **FR-6:** Existing issue-field behavior, keyboard shortcuts, attachment
  handling, pipeline configuration, submission, and edit-mode section ordering
  must not change.
- **FR-7:** Automated regression coverage must verify the panel's scroll-region
  contract and that create controls belong to that region.

## Out of Scope

- Redesigning or reordering create-issue fields.
- Making the header or submit action sticky.
- Changing application-wide mobile navigation or viewport sizing.
- Backend, database, API, deployment, or homelab configuration changes.
- Updates to any service other than Vibe Kanban.

## Acceptance Criteria

- [ ] At a constrained panel height, a user can scroll from the first issue
      fields through the final Create Issue action.
- [ ] Pipeline controls and draft-workspace settings are not clipped when they
      make the create form taller than the available panel height.
- [ ] The panel header remains visible while the body scrolls.
- [ ] Create and edit modes preserve their existing field and section order.
- [ ] A rendered-component regression test verifies the shell, scroll region,
      and containment of create controls.
- [ ] Focused frontend verification, repository formatting, and relevant static
      checks pass.

## Open Questions

None. `/speckit.clarify` resolved the initial questions from the reported
behavior, supplied screenshot, current shared-panel composition, root technical
spec, and prior-knowledge review. Decisions are recorded in
`clarifications.md`.
