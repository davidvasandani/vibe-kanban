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
description → Pipeline card (both modes) → create-mode blocks
(draft-workspace toggle, create button) → SpecKit → Relationships →
Sub-issues → Comments. Edit-mode sections are gated
`!isCreateMode && issueId && renderXxxSection` — keep the whole guard when
moving a block. Exception: `renderPipeline` renders ungated in both modes;
the *container* branches per mode (create: stash-until-submit; edit: seeded
card + "Update Issue" apply button — see
[task-pipeline-block.md](task-pipeline-block.md)) and may return null.

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

## Surfacing an external-connector link (Jira badge) in the panel

An external-connector link (see
[external-connector-sync.md](external-connector-sync.md)) surfaces in **two**
UI places, and they must not diverge: the kanban card
(`KanbanCardContent.tsx`) and the issue detail panel
(`KanbanIssuePanel.tsx`). Both use the **same** three things:

- one presentational leaf, `packages/ui/src/components/JiraBadge.tsx`
  (an `<a target="_blank">` with the issue key; dims on `active={false}`);
- one prop shape, `jiraLink?: { issueKey: string; url: string; active: boolean }
  | null` — a plain **data prop**, not a render prop, because it's a leaf, not a
  container-owned section;
- one lookup, `getJiraLinkForIssue(issueId)` from `useProjectContext()`
  (backed by the `PROJECT_JIRA_LINKS_SHAPE` shape). `active` is derived
  identically on both sides: `link.link_state === 'active'`.

The panel renders the badge in the **header** id-group (next to `displayId`,
after the copy-link button), gated `!isCreateMode && jiraLink` — a create-mode
issue has no persisted link, and this avoids a new bordered section (so none of
the border-convention pitfalls above apply). The container passes
`jiraLink={mode === 'edit' ? jiraLink : undefined}`. Because the URL is already
on the client via the link shape, this is pure presentational wiring — **no
backend/schema/type change** is needed to surface a connector link in a new
place.

## Contributed by
- vk/b37f-move-issue-works
- vk/77eb-vk-pipeline
- vk/a793-vk-jira-bi-direc
