# Feature Specification: Discoverable mobile workspace right drawer

**Feature dir**: `specs/vk/a5f8-concat-repeating/`
**Status**: Clarified

## Summary

Make the existing workspace right drawer obvious and directly reachable on
mobile. The drawer already exists behind a mobile destination represented only
by a Git glyph; users need a control whose visual and accessible meaning matches
the right sidebar they expect from desktop.

## User Stories

- As a mobile workspace user, I want to recognize where the right drawer is so
  that I can inspect repository and workspace details without guessing which
  icon contains them.
- As a keyboard or screen-reader user, I want the drawer control to announce its
  purpose and selection state so that I can navigate the mobile workspace tabs.
- As a returning user, I want my existing mobile tab preference to keep working
  so that this discoverability improvement does not reset my view.

## Functional Requirements

- FR-1: Mobile workspace navigation MUST include a control that is visibly
  identifiable as the right sidebar/drawer.
- FR-2: Activating that control MUST display the existing workspace right
  sidebar content for the selected workspace.
- FR-3: The control MUST expose an accessible name that identifies the right
  sidebar and MUST expose whether it is currently selected.
- FR-4: The control MUST remain reachable in the mobile navigation at the
  narrowest supported phone width.
- FR-5: Existing saved selection of the current mobile drawer destination MUST
  remain valid after the change.
- FR-6: The mobile change MUST NOT alter the drawer's content, collapsible
  section behavior, scrolling, or desktop visibility/persistence behavior.
- FR-7: When there is no selected workspace or the application is creating a
  workspace, the control MUST NOT claim to display usable right-sidebar content.
- FR-8: All other mobile workspace destinations MUST retain their current
  selection, rendering, and state-preservation behavior.

## Out of Scope

- Redesigning or renaming individual sections within the workspace right
  sidebar.
- Introducing a new overlay drawer, backend API, or persisted preference.
- Changing desktop sidebar sizing, toggling, or resizable panels.
- Modifying another service or homelab deployment configuration.

## Acceptance Criteria

- [ ] At a mobile workspace viewport, the top navigation contains a
  right-sidebar control using the established right-sidebar visual metaphor.
- [ ] The control has an accessible `Right sidebar` name and reports selected
  state when its content is active.
- [ ] Activating the control selects the existing mobile drawer destination and
  renders the shared workspace right-sidebar composition.
- [ ] Existing persisted `git` mobile-tab values continue to select the drawer.
- [ ] At narrow phone width the control remains reachable through the existing
  navigation overflow behavior.
- [ ] Automated component tests cover label, icon/semantics, selected state, and
  activation.
- [ ] Relevant frontend type checks, tests, lint, and formatting pass.

## Clarified Decisions

- The visible label is `Sidebar`. It is concise enough for the mobile tab strip
  while accurately identifying the surface. The accessible name is the fuller
  `Right sidebar`.
- The control is omitted when no workspace is selected, including create mode
  and the workspace-less landing. A disabled destination would occupy scarce
  mobile navigation space without providing an action.

## Open Questions

None.

