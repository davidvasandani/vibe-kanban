# Technical Specification: Mobile Workspace Floating Context Bar

**Task ID:** `vk/2792-vk-workspace-flo`
**Status:** Draft for implementation
**Area:** Workspace chat UI (`packages/web-core`, `packages/ui`)

## Problem

The control highlighted in the supplied iPhone screenshot is Vibe Kanban's
workspace **context bar**. It is a floating shortcut palette for desktop
workspace actions such as opening changes, logs, preview, and Git-related
views. It is rendered over the workspace conversation and can be dragged
between six snap points with a mouse.

The context bar is not intended to appear in the mobile workspace layout,
which already exposes the same destinations in the mobile navigation. Its
current visibility guard uses physical-device detection
(`isRealMobileDevice()`), while workspace layout selection uses the responsive
`useIsMobile()` breakpoint. When those signals disagree—for example in an iOS
browser/PWA mode whose user agent is not recognized—the desktop-only context
bar remains visible in the mobile layout. Its drag implementation only handles
mouse events, so touch users cannot move it, and the absolutely positioned bar
obscures chat content.

## Goal

Keep the desktop floating context bar unchanged while ensuring it is never
rendered when Vibe Kanban is using its mobile workspace layout.

## Non-goals

- Adding touch dragging to the context bar.
- Redesigning the context bar or mobile navigation.
- Changing the mobile breakpoint.
- Changing context-bar action definitions or persisted desktop snap position.
- Altering native OS/browser accessibility overlays.

## Proposed Change

1. In `ContextBarContainer`, use the same responsive mobile signal used by
   `WorkspacesLayout`.
2. Return `null` whenever the responsive mobile layout is active.
3. Preserve the existing physical-device guard as defense in depth so a phone
   cannot show the context bar if viewport detection temporarily reports a
   desktop width.
4. Cover the visibility decision with a small unit-testable predicate or
   component test, including the mismatch that caused the defect:
   responsive-mobile `true`, physical-mobile `false`.

## Functional Requirements

### FR-1: Identify the control

The implementation and user-facing handoff must identify the highlighted
control as the Vibe Kanban workspace context bar, a desktop action shortcut
palette.

### FR-2: Hide in responsive mobile layout

When the application selects its mobile layout, the context bar must not be
mounted, regardless of user-agent/device detection.

### FR-3: Retain physical-mobile protection

When physical-device detection reports a real mobile device, the context bar
must remain hidden even if responsive layout detection reports non-mobile.

### FR-4: Preserve desktop behavior

When neither responsive nor physical-device detection reports mobile, the
context bar must render with the existing actions, snap position, persistence,
and mouse dragging behavior.

### FR-5: Avoid duplicate mobile controls

Mobile users must continue to use the existing top mobile navigation tabs for
workspace, chat, changes, logs, preview, browser, and Git destinations; no
replacement floating control is required.

## Acceptance Criteria

1. At a mobile viewport, the workspace chat contains no floating context bar.
2. The bar remains absent when responsive detection is mobile but
   `isRealMobileDevice()` is false.
3. The bar remains absent when `isRealMobileDevice()` is true.
4. At a desktop viewport on a non-mobile device, the bar still renders and its
   mouse drag/snap behavior is unchanged.
5. Existing mobile workspace navigation remains visible and functional.
6. Relevant frontend type checks, tests, lint, and formatting pass.

## Technical Notes

- `WorkspacesLayout.tsx` already branches on `useIsMobile()` and should remain
  the source of truth for responsive workspace mode.
- `ContextBarContainer.tsx` currently performs the physical-device-only guard.
- The bar's current drag hook listens to `mousedown`, `mousemove`, and
  `mouseup`; hiding the redundant control on mobile is preferable to expanding
  its interaction model.
- No API, database, shared generated type, or persistence schema changes are
  expected.

## Risks and Mitigations

- **Breakpoint disagreement:** Use the same responsive hook as the workspace
  layout and keep the physical-device guard.
- **Hook ordering:** Call responsive hooks unconditionally before an early
  return to preserve React hook rules.
- **Desktop regression:** Limit the change to the visibility condition and add
  tests for the desktop case.

## Verification

- Run focused unit tests for the context-bar visibility rule.
- Run the relevant frontend type check and lint targets.
- Run repository formatting as required by `AGENTS.md`.
- Inspect the final diff and independently review it for mobile/desktop
  regressions.
