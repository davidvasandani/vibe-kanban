# Feature Specification: Right Drawer Expand to Available Space

**Feature dir**: `specs/vk/b74a-right-drawer-exp/`
**Status**: Draft

## Summary

Allow expanded workspace right-drawer sections to consume the vertical space
available in the drawer instead of stopping at an arbitrary per-section height
cap. This makes section content easier to inspect while preserving immediate
access to the other section headers.

## User Stories

- As a user inspecting one right-drawer section, I want it to use the available
  drawer height so that I can see more content with less scrolling.
- As a user with several expanded sections, I want them to share the available
  height so that no section is artificially constrained while unused space
  remains.
- As a user navigating the drawer, I want collapsed sections and all section
  headers to remain compact and reachable.

## Functional Requirements

- FR-1: Visible right-drawer sections must remain ordered and top-justified.
- FR-2: A collapsed section must consume only the space required by its header.
- FR-3: An expanded section must be eligible to grow into the drawer's unused
  vertical space.
- FR-4: Multiple expanded sections must participate equally in sharing
  available vertical space, without a fixed or viewport-derived maximum height.
- FR-5: When expanded content exceeds its allocated space, the content area
  must scroll while the section header remains visible.
- FR-6: Existing section visibility, ordering, persisted expansion state,
  header actions, separators, and content behavior must remain unchanged.
- FR-7: Any shared-component behavior introduced for this layout must be
  opt-in so existing uses preserve their current sizing.

## Out of Scope

- Changing the content or ordering of right-drawer sections.
- Changing expansion defaults or persistence semantics.
- Applying the layout to unrelated panels, drawers, or services.
- Modifying deployment or homelab configuration.

## Acceptance Criteria

- [ ] The right drawer contains no artificial per-section maximum-height cap.
- [ ] With one expanded section, it can fill the remaining drawer height.
- [ ] With multiple expanded sections, each can grow and shrink within the
      remaining drawer height.
- [ ] Collapsing a section returns it to header-only intrinsic height and makes
      that space available to expanded siblings.
- [ ] Overflowing section content scrolls independently without scrolling its
      header out of view.
- [ ] Automated component coverage proves expanded, collapsed, and default
      shared-component sizing behavior.
- [ ] Relevant frontend checks and formatting pass.

## Open Questions

None.
