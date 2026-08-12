# Research: Expand Mobile Workspace Toolbar

## Existing behavior

`Navbar.tsx` renders a mobile top row with `justify-between`. The non-project
leading child is an intrinsic-width horizontal scroller containing an
intrinsic-width tab group. The trailing child already uses `shrink-0`. Because
the leading child does not grow, surplus width is not assigned to the tools.

## Decision: explicit flexible owner and distributed children

Use a flexible, zero-minimum leading region and let its tab group occupy at
least the region width. Give individual tabs equal flexible growth with a
minimum width. This satisfies both desired states:

- surplus width expands the controls evenly; and
- insufficient width preserves usable controls via horizontal scrolling.

## Alternatives considered

- **`justify-around` / `justify-evenly` only**: distributes gaps but leaves tap
  targets narrow and does not make the tools themselves fill the region.
- **Remove overflow and let tabs shrink indefinitely**: can make controls too
  small and risks crowding the persistent trailing actions.
- **Calculate widths in JavaScript**: unnecessary, viewport-sensitive, and less
  robust than the browser's flex layout.
- **Change `NavbarContainer`**: violates the existing boundary because the issue
  is entirely internal presentation in the shared `Navbar` primitive.

## Dependencies

No new dependencies.
