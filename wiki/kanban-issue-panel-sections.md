# Kanban issue panel: section slots, ordering, and border convention

The issue detail/create panel on the kanban board is the presentational
component `packages/ui/src/components/KanbanIssuePanel.tsx`. Its container
(`packages/web-core/src/pages/kanban/KanbanIssuePanelContainer.tsx`) supplies
content via render props (`renderWorkspacesSection`, `renderSpecKitSection`,
`renderRelationshipsSection`, `renderSubIssuesSection`,
`renderCommentsSection`, `renderPipeline`); **section order is owned entirely
by the panel component** — reordering sections is a JSX move there, never a
container change. The panel is shared by local-web and remote-web, so one
edit reorders both frontends.

## Layout (top to bottom, inside the scrollable content)

Property row → tags row → Workspaces box (edit mode) → title →
description → create-mode blocks (pipeline, draft-workspace toggle, create
button) → SpecKit → Relationships → Sub-issues → Comments. Edit-mode
sections are gated `!isCreateMode && issueId && renderXxxSection` — keep the
whole guard when moving a block.

## Border convention (one separator per boundary)

- The header rows near the top (property row, tags row) draw `border-b`.
- The trailing sections (Workspaces originally, Relationships, Sub-issues,
  Comments) are wrapped in `<div className="border-t">`; the SpecKit section
  is unwrapped and draws its own border because it renders `null` entirely
  for non-SpecKit tasks.
- Gotcha: when moving a section across the title/description block, flip its
  wrapper border to match its new neighbors. A `border-t` section placed
  directly under a `border-b` row doubles the separator — task b37f moved
  the Workspaces box above title/description and changed its wrapper
  `border-t` → `border-b` for exactly this reason. The section *below* the
  gap still draws its own top border, so nothing is lost at the old spot.

## Testing section order (rendered-DOM component test)

`@vibe/ui` component tests live in `packages/remote-web/src/test/*.test.tsx`
(jsdom + testing-library; see `SessionChatBox.test.tsx`). Recipe from
`KanbanIssuePanel.test.tsx`:

- Render the panel with stub render props
  (`renderWorkspacesSection={() => <div data-testid="…" />}`) and assert
  relative order with
  `a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING`.
- **`NODE_ENV` gotcha**: the dev environment exports
  `NODE_ENV=production`, which makes testing-library fail with
  "act(...) is not supported in production builds of React". Run tests via
  the package script (`pnpm test`, which sets `NODE_ENV=test`) or prefix
  `NODE_ENV=test` when invoking `vitest` directly.
- Without an i18n provider `t()` returns raw keys — match keys or use
  aria-labels/testids, not translated strings.
- Prove an order test bites: `git stash push <component file>`, test must
  fail, `git stash pop`.

## Contributed by
- vk/b37f-move-issue-works
