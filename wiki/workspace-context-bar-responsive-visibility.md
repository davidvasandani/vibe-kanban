# Workspace context bar responsive visibility

The floating workspace context bar is desktop-only, optional chat chrome. It
provides shortcuts to workspace-adjacent views and uses mouse-based dragging
between persisted snap positions. Mobile already exposes the overlapping
destinations through its navbar tabs, so the context bar should not be made
touch-draggable or duplicated there.

## Two mobile signals serve different purposes

Workspace layout selection is owned by `useIsMobile()`, which uses the
project's `max-width: 767px` media query. Physical-device detection through
`isRealMobileDevice()` is a separate, user-agent-derived safeguard.

These signals can disagree. An iOS browser/PWA or embedded client can enter the
responsive mobile layout while physical-device detection returns false.
Conversely, a real mobile device can temporarily report a desktop-sized
viewport. For layout-specific chrome, use the layout signal as the authority
and retain physical-device detection only as defense in depth.

The context-bar visibility truth table is therefore:

| Responsive mobile | Physical mobile | Render context bar |
| --- | --- | --- |
| false | false | yes |
| true | false | no |
| false | true | no |
| true | true | no |

## Component boundary

Keep this policy in
`packages/web-core/src/pages/workspaces/ContextBarContainer.tsx`. The
presentational `packages/ui/src/components/ContextBar.tsx` should remain
layout-agnostic.

Use a thin responsive/platform gate around a desktop-only inner container. This
avoids conditionally calling the desktop action, preference, and positioning
hooks while also avoiding mounting those systems on mobile. Keep
`WorkspacesMainContainer.hideContextBar` as an independent composition
override; carousel workspaces use it for a different reason.

Test the pure truth table directly, especially the mismatch case
`responsive mobile = true`, `physical mobile = false`. Preserve the desktop
drag, snap, and persisted-position implementation unchanged.

## Contributed by

- vk/2792-vk-workspace-flo
