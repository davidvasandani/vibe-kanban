# Responsive flex toolbars

Tags: `vk/2163-fix-toolbar`

## Flexible middle, fixed edge

For a one-line mobile toolbar with a variable set of primary controls and fixed
status/account actions, make ownership of the remaining inline space explicit:

```text
row:               flex
primary region:    flex-1 min-w-0 overflow-x-auto
primary group:     flex flex-1 min-w-fit
primary controls:  flex-1 <usable minimum width>
trailing region:   shrink-0
```

The zero minimum on the primary region is load-bearing. Without it, the flex
item's automatic minimum content width can force the trailing actions outside
the viewport instead of containing overflow in the intended scroller.

The group and controls need both halves of the sizing contract. Flexible growth
shares surplus width across the actual tap targets, while intrinsic/minimum
width prevents the controls from collapsing below usability. When that minimum
total no longer fits, the owning region scrolls horizontally.

## Keep layout ownership at the presentation boundary

Vibe Kanban's mobile navbar state and action availability are composed in
`packages/web-core`, but internal toolbar layout is owned by the shared
`packages/ui` `Navbar`. Fix sizing there so local and remote consumers remain
consistent and route containers do not duplicate presentation policy.

Apply flexible sizing only to the workspace branch when project headers have a
different composition. Safe-area padding belongs to the outer row and should
not be removed to reclaim space; expansion happens inside the already safe
content area.

## Deterministic regression coverage

JSDOM cannot prove pixel geometry. Render the real shared component through an
established consumer test lane and assert the structural contract instead:

- the primary region grows, has a zero minimum, and owns overflow;
- the group and tabs grow while tabs retain their minimum width;
- the trailing region does not shrink; and
- the active tab keeps its semantic state.

This protects the browser-relevant flex relationship without brittle viewport
measurements.
