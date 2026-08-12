# Research: discoverable mobile workspace right drawer

## Existing implementation

- `packages/ui/src/components/Navbar.tsx` defines `MOBILE_TABS`; the final
  `git` entry currently uses `GitForkIcon` and label `Git`.
- `packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx` renders the
  shared `RightSidebar` when `mobileTab === 'git'`.
- `packages/web-core/src/shared/stores/useUiPreferencesStore.ts` persists the
  `git` identifier as part of `MobileTab`.
- Mobile rendering bypasses desktop navbar action items, so the desktop
  `ToggleRightSidebar` action is not a mobile affordance.
- The mobile tab strip already scrolls horizontally and hides visible labels
  below 480px.

## Decisions

### Reuse the existing destination

Keep the `git` identifier and change its presentation. This is the smallest
compatible fix and avoids duplicating the drawer or migrating preferences.

### Use explicit tab semantics

Add an accessible label and selected state directly to tab buttons. This makes
the icon-only narrow layout understandable to assistive technology and gives
tests a stable user-facing contract.

### Filter availability in the container

The shared navbar cannot know whether workspace data exists. Local
`NavbarContainer` already owns selected-workspace/create-mode context, so it is
the correct boundary for omitting an unusable drawer tab.

## Alternatives rejected

- Rendering the desktop `ToggleRightSidebar` action on mobile: mobile ignores
  desktop action sections and uses mutually exclusive destinations, not
  side-by-side visibility.
- Adding an overlay drawer: duplicates `RightSidebar`, adds competing state,
  and conflicts with the established mobile tab model.
- Renaming the state id from `git` to `sidebar`: requires preference migration
  without delivering additional user value.

## Dependencies

No new dependencies.

