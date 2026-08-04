# Implementation Plan: Right Drawer Expand to Available Space

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context

The feature is a React 18 and TypeScript layout change using the repository's
Tailwind utility classes. `RightSidebar.tsx` in `packages/web-core` composes
sections rendered by the shared `CollapsibleSectionHeader.tsx` primitive in
`packages/ui`. The shared primitive owns the actual expanded state, including
localStorage persistence, so it must also own the expanded/collapsed sizing
class decision. No backend, API, data persistence, generated types, or new
dependency is involved.

## Architecture & Approach

1. Add an optional `fillAvailableSpace` presentation prop to
   `packages/ui/src/components/CollapsibleSectionHeader.tsx`.
2. When opted in, derive root sizing from the component's authoritative
   `isExpanded` value:
   - expanded: a zero basis, positive grow, shrinkable minimum height;
   - collapsed: intrinsic height, no growth, no shrink.
3. Preserve the current root classes when the prop is omitted, keeping all
   unrelated shared-component callers unchanged.
4. In `packages/web-core/src/pages/workspaces/RightSidebar.tsx`, make the drawer
   and divided section stack a full-height, shrinkable flex column; remove the
   wrapper with `max-h-[max(50vh,400px)]`; render each header directly as a flex
   child with `fillAvailableSpace` enabled.
5. Retain the body as `overflow-auto` and add `min-h-0`, completing the bounded
   flex chain from drawer through section root to content scroller.
6. Add a rendered-DOM test in `packages/web-core`, whose Vitest setup already
   provides jsdom, to assert opt-in expanded/collapsed root classes and unchanged
   default behavior.

## Data Model

No application data model changes. See `./data-model.md`.

## Contracts

One internal React component interface changes. See `./contracts/component.md`.

## Research Notes

See `./research.md`. No new dependency is needed.

## Constitution Check

- I/III/VI: the approach is a small opt-in extension of the component that
  already owns expansion state; it avoids duplicated state or CSS parent hacks.
- II: rendered-DOM coverage checks the shared component contract.
- IV: presentation remains in `packages/ui`; `web-core` supplies feature layout
  intent without reimplementing collapse behavior.
- XXII: expanded state and flex participation share one owner, the flex chain is
  shrinkable, and the content body remains the only overflow owner.
- No deviations or open constitution questions.

## Risks & Dependencies

- Tailwind class generation must see the new literal utility strings; keeping
  full class names in source satisfies static scanning.
- Equal `basis-0 flex-1` shares can make content shorter than its intrinsic size;
  `min-h-0` and body overflow are required to prevent the content from forcing
  its section larger.
- The non-collapsible Issue section is always expanded and therefore joins the
  same pool, preventing it from starving other sections.
- Shared UI changes affect local and remote frontends, but the new behavior is
  opt-in and only enabled by the workspace right drawer.
