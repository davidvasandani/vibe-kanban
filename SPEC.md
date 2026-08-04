# Right Drawer Expand to Available Space

## Summary

Update the workspace right drawer so its visible sections are top-justified and
expanded section bodies can use the drawer's remaining vertical space. The
drawer must not impose the current artificial per-section maximum height.

## Problem

Each right-drawer section is currently constrained by
`max-h-[max(50vh,400px)]`. This prevents an expanded section from growing into
otherwise unused drawer space and can create unnecessary nested scrolling.

## Desired behavior

- Visible section headers remain stacked from the top of the right drawer.
- Collapsed sections consume only their header height.
- Expanded sections share the vertical space left after visible headers and
  intrinsically sized non-collapsible content.
- A single expanded section may grow to fill all available remaining space.
- Multiple expanded sections divide the available space without any fixed or
  viewport-derived maximum height.
- When expanded content needs more room than its allocated share, that
  section's body scrolls without forcing headers out of view.
- Existing visibility, persisted expansion state, actions, borders, and content
  rendering remain unchanged.

## Scope

The change is limited to the Vibe Kanban web UI's workspace right drawer. No
other service or homelab deployment configuration needs modification.

## Acceptance criteria

1. No right-drawer section wrapper uses the existing
   `max-h-[max(50vh,400px)]` constraint or an equivalent artificial cap.
2. The section stack occupies the drawer height and distributes spare vertical
   space among expanded sections.
3. Collapsed headers remain compact and top-justified.
4. Overflowing content remains independently scrollable inside its expanded
   section.
5. Automated coverage verifies the flex sizing behavior, including the
   expanded and collapsed wrapper states.
6. Relevant frontend formatting, type checks, lint, and focused tests pass.

## Non-goals

- Changing which right-drawer sections are shown.
- Changing default or persisted expansion state.
- Redesigning section content or headers.
- Modifying another service or deployment configuration.
