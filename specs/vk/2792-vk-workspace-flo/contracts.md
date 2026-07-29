# Contracts: Hide Workspace Context Bar on Mobile Layout

**Feature dir**: `specs/vk/2792-vk-workspace-flo/`
**Task**: `vk/2792-vk-workspace-flo`
**Spec**: [`spec.md`](spec.md)

## External Contracts

No HTTP API, database, generated TypeScript, MCP, filesystem, or persistence
contract changes.

No changes are expected to:

- `shared/types.ts`
- `shared/remote-types.ts`
- SQLx migrations
- backend routes
- context-bar action definitions
- UI preference storage schema

## Frontend Visibility Contract

The workspace context bar render policy is:

| Responsive workspace layout | Physical-device signal | Expected result |
| --- | --- | --- |
| mobile | mobile | context bar not mounted |
| mobile | non-mobile/unknown | context bar not mounted |
| desktop | mobile | context bar not mounted |
| desktop | non-mobile/unknown | context bar mounted normally |

The responsive workspace layout signal is the existing `useIsMobile()` hook.
The physical-device signal is the existing `isRealMobileDevice()` behavior or
its hook equivalent.

## Component Boundary Contract

`WorkspacesMainContainer` continues to provide `contextBarContent` only when:

- a workspace/session exists;
- `hideContextBar` is false;
- the context-bar container's own visibility policy allows rendering.

`hideContextBar` remains an explicit composition override. It is not replaced by
the mobile policy.

`ContextBarContainer` continues to expose the same prop contract:

```typescript
type ContextBarContainerProps = {
  containerRef: RefObject<HTMLElement | null>;
};
```

`packages/ui/src/components/ContextBar.tsx` keeps its existing props and does
not receive mobile, responsive-layout, or physical-device props.

## Desktop Behavior Contract

When responsive mobile is false and physical-device mobile is false, the
context bar must preserve:

- existing primary and secondary action groups;
- action visibility and enabled-state filtering;
- tooltips and shortcuts;
- action execution behavior;
- IDE icon and copy special-item behavior;
- desktop placement style from `useContextBarPosition(containerRef)`;
- mouse drag behavior;
- snap-position persistence.

## Mobile Behavior Contract

When responsive mobile is true, the context bar must not be mounted or visible,
even if `isRealMobileDevice()` is false.

When physical-device mobile is true, the context bar must not be mounted or
visible, even if responsive layout would otherwise be desktop.

Mobile users continue to use the existing workspace mobile navigation tabs for
workspaces, chat, changes, logs, preview, browser, and Git. This feature does
not add a replacement floating control.

## Test Contract

Focused automated coverage must verify the visibility truth table:

- responsive mobile true and physical mobile false hides the bar;
- responsive mobile false and physical mobile true hides the bar;
- both mobile signals true hides the bar;
- both mobile signals false renders the bar.

Regression checks should also cover, manually or with a smoke test:

- desktop action set still appears on desktop;
- desktop drag/snap/persist behavior still works;
- mobile navigation remains available for destinations that overlap with the
  context bar.

## Non-Contracts

The following are explicitly out of contract for this feature:

- changing the mobile breakpoint;
- adding touch dragging to the context bar;
- redesigning mobile navigation;
- redesigning the desktop context bar;
- changing action destinations or route behavior;
- changing saved context-bar position values.
