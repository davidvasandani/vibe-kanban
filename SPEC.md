# Fix Toolbar — Technical Specification

## Summary

Improve the mobile workspace navbar so the workspace tool tabs use the available
horizontal space instead of remaining a tightly packed cluster with unused space
on the leading side. The right-side deployment, settings, command, and user
controls must remain visible and stable.

## Problem

On phone-sized workspace views, the primary tool tabs (Chat, Diff, Logs,
Preview, Browser, and Sidebar, subject to route availability) are rendered at
their content width. In landscape/mobile layouts this leaves visually unused
horizontal space and makes the controls feel compressed, as shown in the task
screenshots.

## Scope

- Update the Vibe Kanban frontend mobile navbar layout only.
- Allow the tool-tab strip to grow across all horizontal room available between
  its leading navigation affordance and the fixed right-side controls.
- Distribute the available width among visible tool tabs while retaining a
  usable fallback when the viewport is too narrow.
- Preserve existing tab visibility, active state, labels, click behavior,
  accessibility attributes, safe-area/window-control clearance, and navigation
  actions.
- Add focused regression coverage for the mobile navbar layout contract.

## Out of Scope

- Desktop navbar changes.
- Changes to tool availability or tab ordering.
- Changes to the homelab deployment configuration or any other service.
- Visual redesign of icons, colors, typography, or right-side controls.

## Functional Requirements

1. The mobile workspace toolbar's leading section MUST consume the horizontal
   space remaining after the right-side controls.
2. The visible tool tabs MUST expand within that section so unused width is
   shared across the controls.
3. When the available width cannot fit the controls at their usable minimum
   size, the toolbar MUST remain horizontally scrollable and MUST NOT push the
   right-side controls off-screen.
4. Project-page mobile headers MUST retain their current layout.
5. Existing safe-area and iPad window-control spacing MUST remain effective.
6. The active tool MUST retain its visible active indicator and `aria-pressed`
   state.

## Acceptance Criteria

- In a landscape phone workspace view, the tool buttons occupy the available
  toolbar region rather than appearing as a compact group separated from empty
  space.
- Settings, command-bar, deployment status, sync status, and user controls remain
  fixed at the trailing side and usable.
- On narrower viewports, all available tabs can still be reached by horizontal
  scrolling without wrapping.
- Mobile project-page behavior and desktop navbar behavior are unchanged.
- Automated tests assert the growing/distributed toolbar classes and the
  fixed-width trailing section.
- Relevant frontend checks, formatting, and tests pass.

## Implementation Notes

The likely change is localized to `packages/ui/src/components/Navbar.tsx`.
Prefer flexbox growth on the non-project mobile toolbar region, an inner
full-width tab group, and equal growth for tab buttons. Preserve `min-w-0` on
the flexible region and `shrink-0` on trailing controls to ensure overflow is
contained in the toolbar rather than the navbar.
