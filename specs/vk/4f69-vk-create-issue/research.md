# Research: Scrollable Create-Issue Settings

## Sources Reviewed

- Supplied mobile screenshot
- Root `SPEC.md`, workspace `PRIOR_KNOWLEDGE.md`, and `clarifications.md`
- `.specify/memory/constitution.md`
- `packages/ui/src/components/KanbanIssuePanel.tsx`
- `packages/web-core/src/pages/kanban/KanbanIssuePanelContainer.tsx`
- `packages/web-core/src/pages/kanban/ProjectKanban.tsx`
- `packages/web-core/src/pages/kanban/ProjectRightSidebarContainer.tsx`
- `packages/remote-web/src/test/KanbanIssuePanel.test.tsx`
- `docs/knowledge-base/worktree-formatting-prerequisites.md`

## Findings and Decisions

### Scroll ownership already exists

The mobile host and desktop right-panel host both use `h-full overflow-hidden`.
`KanbanIssuePanel` uses `flex flex-col h-full overflow-hidden`; its header is
`shrink-0`, and the next child is `flex-1 overflow-y-auto`. The intended owner
of vertical scrolling is therefore unambiguous: the body, not the page.

Decision: preserve that ownership and correct the flex sizing locally.

### Flex automatic minimum size is the mismatch

A column flex item's automatic minimum height is content-dependent. A body that
is `flex-1` can consequently refuse to shrink to the remaining height even when
it declares `overflow-y-auto`, leading an ancestor's `overflow-hidden` to clip
the excess. `min-h-0` explicitly permits the flex item to shrink, after which
its overflow produces the intended internal scrollbar.

Decision: add `min-h-0` to the body. No JavaScript measurement or viewport unit
is needed.

### Control placement should remain unchanged

Pipeline controls, draft-workspace toggle, and Create Issue button are already
inside the body. Making the submit action sticky would change behavior and
available body height, while moving settings would change ordering.

Decision: keep all controls in their existing DOM locations.

### Test the explicit contract

The existing remote-web test suite renders `KanbanIssuePanel` and checks section
order. JSDOM cannot compute scroll height, so a pixel-scroll assertion would be
false confidence. Stable test IDs can identify the shell and body without
depending on incidental DOM indexes.

Decision: assert the utility-class contract and descendant relationships in the
rendered DOM.

### Dependencies and generated artifacts

No new dependency, API, database entity, generated type, translation, or
deployment setting is required.

## Alternatives Rejected

- `max-h-screen`/`100vh`: duplicates the host's height authority and is fragile
  around mobile browser chrome.
- Make the page scroll: would move the header and leak panel content outside its
  intended boundary.
- Sticky footer: unrequested interaction/layout change.
- JavaScript ResizeObserver/height calculation: unnecessary complexity for a
  native flexbox sizing constraint.
