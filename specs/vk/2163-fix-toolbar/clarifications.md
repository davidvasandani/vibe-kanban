# Clarifications: Expand Mobile Workspace Toolbar

## Q1. How should surplus width be allocated?

**Decision**: Distribute surplus width equally across the visible workspace
tool tabs.

**Basis**: The task explicitly asks to "expand the tools to fill the empty
space," which calls for the controls themselves—not only an invisible wrapper—to
use the available region. Equal distribution produces predictable tap targets
as route-dependent tabs appear or disappear.

**Guardrail**: Expansion is bounded to the flexible workspace-tool region. Each
tab retains a usable minimum width, and the region scrolls horizontally when
that minimum cannot fit, so persistent trailing controls are not displaced.

## Remaining Questions

None.
