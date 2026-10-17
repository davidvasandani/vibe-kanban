# Feature Specification: Keep Mobile Toolbar Leading Tool Visible

**Feature dir**: `specs/vk/2163-fix-toolbar-followup/`
**Task id**: `vk/2163-fix-toolbar-followup`
**Status**: Clarified

## Summary

Correct the expanded mobile workspace toolbar so its first tool is not clipped
at the leading edge after another tool is selected.

## User Stories

- As a mobile user, I want every toolbar icon fully visible when the tools fit.
- As a narrow-screen user, I want only the tools to scroll while navigation and
  trailing actions remain fixed.

## Functional Requirements

- FR-1: The first visible tool must be fully visible when all tools fit.
- FR-2: Workspace tools must continue sharing surplus width.
- FR-3: Genuine tool overflow must remain horizontally scrollable.
- FR-4: Leading navigation and trailing actions must remain outside tool
  overflow and retain their current behavior.
- FR-5: Active state, accessibility, safe areas, project headers, and desktop
  behavior must remain unchanged.
- FR-6: Focused component coverage must protect overflow ownership.

## Out of Scope

- Tool ordering/availability changes.
- Any non-Vibe-Kanban service or deployment change.

## Acceptance Criteria

- [ ] The screenshot's partially clipped first tool no longer occurs.
- [ ] Tools still expand evenly and scroll only when necessary.
- [ ] Leading and trailing controls remain fixed and usable.
- [ ] Relevant tests and checks pass.

## Open Questions

None. Only the inner workspace-tool group owns horizontal overflow. Fixed
leading navigation and trailing actions remain outside it.
