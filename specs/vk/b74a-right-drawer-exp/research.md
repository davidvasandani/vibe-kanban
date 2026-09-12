# Research: Right Drawer Flexible Sections

## Decision: size where expansion state is authoritative

`CollapsibleSectionHeader` owns its live `expanded` state and localStorage
persistence. `RightSidebar` only passes a default derived from a separate
preference view and cannot reliably choose parent flex classes after the user
toggles a header. Therefore the shared component receives an opt-in
`fillAvailableSpace` prop and selects its root sizing from `isExpanded`.

Rejected alternatives:

- Duplicate expansion state in `RightSidebar`: creates two state owners and can
  diverge from the shared component's persistence behavior.
- CSS `:has()` parent selection: couples the feature to internal DOM structure,
  obscures the layout contract, and is harder to cover meaningfully.
- Make all `CollapsibleSectionHeader` callers flexible: changes unrelated
  surfaces and violates the request's narrow scope.

## Decision: equal zero-basis flex shares

Expanded sections use equal flex participation so available space is shared
deterministically. A zero basis prevents intrinsic content height from skewing
the allocation. Collapsed sections opt out of growth and shrinking.

## Decision: retain one nested content scroller

The drawer provides the height bound; the section stack and section root allow
shrinking; the expanded body owns normal content overflow. This keeps each
header outside its content scroller. The outer drawer retains vertical overflow
as a defensive fallback so all headers remain reachable when a very short
viewport cannot fit their combined intrinsic height.

## Dependencies

No new runtime or development dependencies.
