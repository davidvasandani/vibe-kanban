# Feature Specification: Expand Mobile Workspace Toolbar

**Feature dir**: `specs/vk/2163-fix-toolbar/`
**Task id**: `vk/2163-fix-toolbar`
**Status**: Clarified

## Summary

Make the workspace tools in the mobile navbar use the available horizontal
space. Users should see a balanced, easy-to-tap toolbar rather than a compact
cluster beside unused room, while persistent status and account actions remain
available.

## User Stories

- As a mobile workspace user, I want the tool tabs to fill the usable toolbar
  area so each destination is easier to identify and tap.
- As a user on a narrow screen, I want every available tool to remain reachable
  without losing settings, status, or account controls.
- As a project user, I want the existing project header layout to remain stable
  because the workspace tool strip does not apply there.

## Functional Requirements

- FR-1: On mobile workspace pages, the tool-tab region must occupy all usable
  horizontal space between its leading navigation controls and the persistent
  trailing controls.
- FR-2: Visible workspace tool tabs must share surplus width across the tool-tab
  region instead of remaining grouped at their intrinsic content width.
- FR-3: If the available width is smaller than the combined usable minimum width
  of the visible controls, the tool-tab region must allow horizontal scrolling.
- FR-4: Trailing deployment status, synchronization status, reload, settings,
  command-bar, and user controls must remain visible, ordered, and usable where
  currently applicable.
- FR-5: Existing tool visibility, ordering, navigation behavior, icons, labels,
  active indicator, and accessibility state must not change.
- FR-6: Existing safe-area and window-control clearance must continue to protect
  controls from operating-system overlays.
- FR-7: Mobile project headers and the desktop navbar must retain their current
  behavior.
- FR-8: Automated regression coverage must protect the mobile workspace
  toolbar's growth, distribution, overflow, and fixed trailing-region contract.

## Out of Scope

- Changing which workspace tools are available.
- Reordering or renaming tools.
- Redesigning icons, colors, typography, or the active-state treatment.
- Changing desktop navbar layout.
- Changing any service other than Vibe Kanban or its deployment configuration.

## Acceptance Criteria

- [ ] In a landscape phone-sized workspace view, visible tool tabs fill the
  available leading toolbar region rather than appearing as a tight cluster.
- [ ] Surplus toolbar width is shared by the visible tool tabs.
- [ ] At constrained widths, tabs remain on one line and can be reached through
  horizontal scrolling without displacing trailing controls.
- [ ] Settings, command-bar, status, and account controls remain on the trailing
  side and retain their behavior.
- [ ] The active tool retains its indicator and `aria-pressed="true"` state.
- [ ] Mobile project pages and desktop navbar rendering remain unchanged.
- [ ] Focused component tests, frontend checks, lint, formatting, and diff
  validation pass.

## Open Questions

None. See `clarifications.md`.
