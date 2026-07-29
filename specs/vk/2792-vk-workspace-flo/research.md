# Research: Hide Workspace Context Bar on Mobile Layout

**Feature dir**: `specs/vk/2792-vk-workspace-flo/`
**Task**: `vk/2792-vk-workspace-flo`
**Spec**: [`spec.md`](spec.md)

## Sources Reviewed

- [`spec.md`](spec.md)
- [`clarifications.md`](clarifications.md)
- `assets/speckit/memory/constitution.md`
- Root `PRIOR_KNOWLEDGE.md`
- `packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx`
- `packages/web-core/src/pages/workspaces/WorkspacesMainContainer.tsx`
- `packages/web-core/src/pages/workspaces/ContextBarContainer.tsx`
- `packages/web-core/src/shared/hooks/useIsMobile.ts`
- `packages/ui/src/components/ContextBar.tsx`
- Existing `packages/web-core` Vitest files for local test style

Note: the requested
`specs/vk/2792-vk-workspace-flo/PRIOR_KNOWLEDGE.md` file does not exist in this
worktree. The root `PRIOR_KNOWLEDGE.md` is task-specific for
`vk/2792-vk-workspace-flo`, so it was used as the fallback prior-knowledge
source.

## Existing Layout Signals

`WorkspacesLayout` owns the workspace responsive split and reads
`useIsMobile()` directly. When true, it renders the mobile tabbed workspace
composition. When false, it renders the desktop panel layout.

`useIsMobile()` is backed by the existing media query:

```text
(max-width: 767px)
```

Decision: reuse `useIsMobile()` as the context-bar responsive visibility
authority. Do not add a second breakpoint and do not infer layout from physical
device alone.

## Existing Physical-Device Guard

`ContextBarContainer` imports `isRealMobileDevice()` and currently returns
`null` only when that function reports true. `buildSpecialItem()` also checks
`isRealMobileDevice()` for the IDE icon branch.

This protects real mobile devices in some cases but does not protect desktop
browsers narrowed to the mobile layout, embedded views, or devices whose user
agent is not recognized.

Decision: retain physical-device detection as defense in depth, but combine it
with responsive layout state for the top-level context-bar visibility decision.

## Context Bar Ownership

The presentational context bar in `packages/ui` receives render items, style,
drag state, and mouse handlers. It does not know about workspace layout, mobile
tabs, action visibility context, or device state.

`ContextBarContainer` in `packages/web-core` already owns action filtering,
action execution, icon adaptation, position state, drag handler wiring, and the
current physical-device guard.

Decision: keep the visibility policy in `packages/web-core`, preferably in or
immediately next to `ContextBarContainer`. Do not add responsive or device
awareness to `packages/ui`.

## Optional Chrome Precedent

`WorkspacesMainContainer` accepts `hideContextBar`. The carousel uses that prop
because context-bar actions resolve against the route-level single-workspace
context and are inappropriate in carousel columns.

Decision: treat the context bar as optional workspace chat chrome. Hiding it in
mobile layout is consistent with existing composition behavior. Do not remove
or repurpose `hideContextBar`.

## Mobile Navigation Coverage

The mobile branch in `WorkspacesLayout` includes tabs for:

- workspaces
- chat
- changes
- logs
- preview
- browser
- Git

Those destinations overlap with the context bar's desktop shortcuts.

Decision: do not introduce a touch-draggable or replacement floating mobile
control. The existing mobile navigation remains the expected access path.

## Test Strategy

The repo already uses Vitest in `packages/web-core`, with many tests structured
as pure function tests. `ContextBarContainer` has several dependencies that are
not relevant to the boolean visibility policy: action execution, user system
config, visibility context, and position persistence.

Decision: make the core policy a pure predicate and test it directly. Add a
component-level test only if implementation wiring cannot be reviewed cleanly
from the diff.

Required predicate truth table:

| Responsive mobile | Real mobile device | Render context bar |
| --- | --- | --- |
| false | false | true |
| true | false | false |
| false | true | false |
| true | true | false |

This directly covers the defect case where responsive mobile is true but
physical-device detection is false.

## Manual Verification Notes

The prior knowledge notes that the task environment does not provide a real
touch engine. Automated tests should therefore lock the visibility contract, and
manual verification should cover:

- narrow desktop browser viewport to validate responsive-mobile behavior
  without physical mobile detection;
- desktop viewport on non-mobile device to validate the context bar still
  renders;
- desktop drag/snap/persist behavior after the visibility gate is added;
- mobile navigation destinations remain available.

## Decisions Summary

- Use `useIsMobile()` as the responsive layout source of truth.
- Preserve `isRealMobileDevice()` as a secondary hiding signal.
- Hide by conditional rendering, not CSS-only display suppression.
- Keep the policy in `packages/web-core`.
- Keep `packages/ui` presentational and layout-agnostic.
- Do not change breakpoint, action definitions, routing, snap persistence,
  APIs, database schema, or generated types.
