# Feature Specification: Hide Workspace Context Bar on Mobile Layout

**Feature dir**: `specs/vk/2792-vk-workspace-flo/`
**Status**: Draft

## Summary

Vibe Kanban's workspace context bar is a desktop shortcut palette for opening
workspace-adjacent views such as changes, logs, preview, browser, and Git
actions. It floats over the workspace conversation and supports mouse-based
dragging between desktop snap positions.

The mobile workspace layout already provides those destinations through the
mobile navigation. The floating context bar is therefore redundant on mobile,
and when it appears there it can obscure chat content and cannot be moved by
touch users. The product should hide the context bar whenever the responsive
workspace layout is mobile, while preserving the existing desktop experience.

## User Stories

- As a mobile workspace user, I want the chat view to be free of desktop-only
  floating controls, so I can read and interact with the conversation without
  obstruction.
- As a mobile workspace user, I want to use the existing mobile navigation for
  changes, logs, preview, browser, and Git destinations, so controls behave the
  way the mobile layout expects.
- As a desktop workspace user, I want the existing context bar to keep working
  as it does today, so my shortcut workflow and saved snap position are not
  disrupted.
- As a maintainer, I want mobile visibility to follow the same responsive signal
  as the workspace layout, so the UI does not depend on fragile physical-device
  detection alone.

## Functional Requirements

- **FR-1:** The highlighted control MUST be treated as the Vibe Kanban
  workspace context bar, a desktop action shortcut palette and optional
  workspace chat chrome.
- **FR-2:** When Vibe Kanban selects the responsive mobile workspace layout, the
  context bar MUST NOT be mounted or visible.
- **FR-3:** The mobile-layout visibility rule MUST apply even when
  physical-device or user-agent detection does not identify the device as a real
  mobile device.
- **FR-4:** When physical-device detection identifies a real mobile device, the
  context bar MUST remain hidden even if the viewport or responsive signal would
  otherwise allow the desktop layout.
- **FR-5:** When neither responsive layout detection nor physical-device
  detection reports mobile, the context bar MUST render with its existing
  actions, placement, persistence, mouse drag behavior, and snap behavior.
- **FR-6:** Mobile users MUST continue to access workspace, chat, changes, logs,
  preview, browser, and Git destinations through the existing mobile navigation;
  this feature MUST NOT introduce a replacement floating mobile control.
- **FR-7:** The visibility policy MUST belong at the workspace composition or
  context-bar container boundary, where layout and platform decisions already
  live.
- **FR-8:** The presentational context bar component MUST NOT need device or
  responsive-layout awareness solely to satisfy this feature.
- **FR-9:** The feature MUST NOT change the mobile breakpoint.
- **FR-10:** The feature MUST NOT change context-bar action definitions,
  routing, desktop snap-position persistence, or any API, database, shared type,
  or persistence schema.

## Out of Scope

- Adding touch dragging to the context bar.
- Redesigning the desktop context bar.
- Redesigning mobile workspace navigation.
- Changing the responsive mobile breakpoint.
- Changing context-bar actions, destinations, or persisted desktop snap
  position.
- Altering native OS, browser, or accessibility overlays.
- Changing API, database, shared generated type, or persistence schemas.

## Acceptance Criteria

- [ ] At a mobile viewport, the workspace chat does not mount or display the
      floating context bar.
- [ ] In the defect case where responsive mobile detection is true but
      physical-device detection is false, the context bar remains absent.
- [ ] When physical-device detection reports a real mobile device, the context
      bar remains absent.
- [ ] At a desktop viewport on a non-mobile device, the context bar still
      renders with the existing action set.
- [ ] Desktop mouse drag, snap positioning, and persisted snap behavior remain
      unchanged.
- [ ] Existing mobile workspace navigation remains visible and functional for
      the destinations that overlap with the context bar.
- [ ] Focused automated coverage verifies the visibility decision, including
      disagreement between responsive-mobile and physical-mobile signals.
- [ ] Relevant frontend type checks, lint, formatting, and tests pass.

## Clarified Decisions

- `useIsMobile()` is the responsive-layout source of truth because
  `WorkspacesLayout` already uses it to select the mobile workspace layout.
- The existing physical-device guard should be retained as defense in depth, not
  treated as the sole mobile visibility rule.
- The context bar is optional workspace chrome. Existing carousel behavior
  already confirms it can be suppressed in contexts where it is not appropriate.
- Mobile should not gain a touch-enabled duplicate of the desktop shortcut
  palette because the mobile navigation already covers the same destinations.
- Layout and platform policy should stay in `packages/web-core`; the shared
  presentational context bar should remain layout-agnostic.

## Success Metrics

- Mobile workspace users no longer see a floating desktop context bar over chat
  content.
- The original mismatch case is prevented: responsive mobile layout plus
  unrecognized physical-device detection still hides the bar.
- Desktop users experience no visible or behavioral regression in the context
  bar.
