# Clarifications: Right Drawer Expand to Available Space

## Resolved

### How should multiple expanded sections divide space?

All expanded, collapsible sections participate equally in the drawer's
remaining vertical space. Collapsed sections consume only their header height.
This directly reflects the request that expanded items be allowed to expand to
give content space, removes the arbitrary cap, and yields deterministic behavior
regardless of content length.

The always-expanded non-collapsible Issue section participates in the same
flexible pool when visible; otherwise its content could intrinsically consume
the drawer and starve collapsible siblings.

## Remaining open questions

None.
