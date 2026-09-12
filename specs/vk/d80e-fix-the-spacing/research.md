# Research: Compact Right Drawer Section Spacing

## Root Cause

`RightSidebar.tsx` passes `fillAvailableSpace` to every section. In the shared
primitive, an expanded opted-in section becomes `flex-1 min-h-0`. Server
Affinity has only two compact grid rows, so on a tall mobile drawer its section
root receives a large equal share of remaining height and the body's flex
layout visually separates the rows across that space.

## Decision

Treat available-height participation as section composition metadata. Server
Affinity opts out through an explicit intrinsic-height primitive mode; existing
content panels retain flexible fill. A distinct mode is required because the
primitive deliberately keeps `h-full min-h-0` when `fillAvailableSpace` is
omitted or false for legacy callers, and this project's class combiner does not
resolve conflicting Tailwind utilities. This keeps disclosure state with its
current owner without changing the shared default contract.

## Alternatives Considered

- **Change the affinity grid or add fixed heights:** rejected because the grid
  already handles label alignment and mobile shrinking, while fixed heights
  are brittle and conflict with intrinsic content.
- **Change `CollapsibleSectionHeader` globally:** rejected because other callers
  and content panels rely on its flexible sizing contract.
- **Duplicate expansion state in `RightSidebar`:** rejected because persisted
  live state belongs to the primitive and duplicate state can diverge.
- **Use CSS selectors against child content:** rejected as brittle coupling to
  internal DOM rather than an explicit composition decision.

## Dependencies

No new runtime or development dependency is needed.
