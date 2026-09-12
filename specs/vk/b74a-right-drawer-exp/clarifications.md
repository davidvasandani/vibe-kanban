# Clarifications: Right Drawer Expand to Available Space

## Resolved

### How should multiple expanded sections divide space?

All expanded, collapsible sections participate equally in the drawer's
remaining vertical space. Collapsed sections consume only their header height.
This directly reflects the request that expanded items be allowed to expand to
give content space, removes the arbitrary cap, and yields deterministic behavior
regardless of content length.

The non-collapsible Issue section remains intrinsically sized when visible. It
has no expand/collapse interaction and its compact content should not receive an
equal share that would leave blank space while flexible siblings are cramped.

## Remaining open questions

None.
