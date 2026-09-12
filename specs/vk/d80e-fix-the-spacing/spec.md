# Feature Specification: Compact Right Drawer Section Spacing

**Feature dir**: `specs/vk/d80e-fix-the-spacing/`
**Task id**: `d80e-fix-the-spacing`
**Status**: Clarified

## Summary

Keep compact workspace drawer sections grouped at their natural height so the
mobile Sidebar does not insert a large blank gap between closely related Server
Affinity controls, while retaining available-height sharing for content panels
that need to grow and scroll.

## User Stories

- As a mobile workspace user, I want Server Affinity values and controls grouped
  together so I can understand and change placement without scanning a tall
  empty area.
- As a desktop workspace user, I want compact drawer controls to remain compact
  while content-heavy sections continue to use the available drawer space.
- As a keyboard or assistive-technology user, I want the existing disclosure
  behavior and header context to remain unchanged.

## Functional Requirements

- FR-1: Expanded Server Affinity content must consume only the vertical space
  required by its controls.
- FR-2: The current-server row and run-on row must remain adjacent with the
  existing compact spacing at desktop and mobile widths.
- FR-3: Drawer sections whose content is intended to grow or scroll must retain
  their existing available-height behavior.
- FR-4: Collapsing any drawer section must continue to hide its body and return
  it to intrinsic header height.
- FR-5: The collapsed Server Affinity header must continue to show its bounded,
  truncating placement context.
- FR-6: Existing Server Affinity labels, placement options, mutation behavior,
  persistence, and responsive access must not change.
- FR-7: Automated coverage must distinguish compact intrinsic-height sections
  from fillable content sections at the rendered drawer boundary.

## Out of Scope

- Redesigning Server Affinity controls or changing placement behavior.
- Changing the shared disclosure primitive's state model.
- Changing other Vibe Kanban services or homelab deployment configuration.
- Assigning fixed pixel heights or viewport-derived caps to drawer sections.

## Acceptance Criteria

- [ ] On a tall mobile viewport, expanding Server Affinity places "Run on"
  immediately below "Current server" with no fill-height blank gap.
- [ ] Expanded Server Affinity does not participate as a flexible remaining-
  height panel.
- [ ] At least one content-oriented drawer section remains a flexible panel when
  expanded.
- [ ] Collapsed sections remain intrinsic and Server Affinity header context
  remains visible.
- [ ] Focused frontend tests, type checks, lint, formatting, and whitespace
  checks pass.

## Open Questions

None. See `clarifications.md` for the resolved sizing scope.
