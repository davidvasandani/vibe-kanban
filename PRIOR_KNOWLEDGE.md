# Prior Knowledge: Right Drawer Section Spacing

The project knowledge base is not empty. The following pages are directly
relevant to this task.

## `wiki/flexible-collapsible-panel-stacks.md`

- `RightSidebar` is a bounded vertical flex stack shared by the desktop drawer
  and mobile Sidebar tab.
- `CollapsibleSectionHeader` owns live expansion state. Callers opt into
  remaining-height participation with `fillAvailableSpace`; an expanded opted-in
  section receives `flex-1 min-h-0`, while a collapsed or non-collapsible
  section is intrinsic.
- The complete `min-h-0` chain and body-level `overflow-auto` are necessary for
  panels whose content should grow and scroll. The outer drawer retains
  vertical overflow as a short-window fallback.
- Regression coverage should assert rendered sizing classes because JSDOM
  cannot calculate real layout.

## `docs/knowledge-base/workspace-affinity-migration.md`

- The expanded Server Affinity body deliberately uses a two-column grid with
  `auto` and `minmax(0, 1fr)` columns so labels stay associated with values and
  controls can shrink at mobile widths.
- Collapsed affinity context comes from the workspace summary and must remain
  in a bounded, truncating header item so the caret stays usable.
- The existing compact body layout is therefore correct; the excessive blank
  space comes from the section's participation in the parent flex stack, not
  from its internal row grid.

## `docs/knowledge-base/nested-flex-scroll-containment.md`

- Flex growth and `min-h-0` should remain on content panels that need to share
  bounded height and scroll.
- Layout regression tests should protect class contracts at stable component
  boundaries rather than rely on pixel measurements in JSDOM.

## Consequences for This Task

1. Keep Server Affinity's internal grid unchanged.
2. Make fill-available-space participation a per-section composition decision;
   Server Affinity should be intrinsic when expanded, while content panels keep
   flexible growth.
3. Do not duplicate disclosure state in `RightSidebar` or alter the shared
   primitive's state ownership.
4. Preserve the drawer's bounded flex/overflow chain for desktop and mobile.
5. Add rendered-DOM coverage at the `RightSidebar` composition boundary to
   prevent compact sections from regaining `flex-1` sizing.
