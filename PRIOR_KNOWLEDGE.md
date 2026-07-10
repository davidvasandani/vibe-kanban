# Prior Knowledge — recalled for `vk/b37f-move-issue-works`

Searched the project knowledge base — `wiki/` (7 topic pages + INDEX) — for
pages relevant to this task (reordering sections inside the kanban issue
detail panel, `packages/ui` + `packages/web-core`). No page covers the
`KanbanIssuePanel` section layout directly; two are adjacent enough to
inform the work.

## Relevant findings

**[wiki/appbar-rail-and-org-tiles.md] — adjacent pattern.** Establishes the
fork's frontend convention this task follows: presentational components in
`packages/ui` own layout and receive behavior via render props/slots from
`packages/web-core` containers; layout changes belong in the `packages/ui`
component, not the container. Also the source of the "don't render a no-op
interactive element" gotcha — reinforces keeping the edit-mode
`!isCreateMode && issueId && renderWorkspacesSection` guard intact when
moving the block.

**[wiki/kanban-items-state-and-activity-grouping.md] — scope boundary.**
Documents the board-side state machinery (items array ↔ sort_order
contract, workspace activity signals). Confirms the issue *panel* layout is
independent of board state — a pure JSX reorder in `KanbanIssuePanel.tsx`
cannot disturb drag-and-drop or activity grouping, so the change surface
stays one file.

## Not relevant

`external-connector-sync.md`, `electric-sync-fallback.md`,
`self-hosted-deployment.md`, `project-context-map.md`,
`mobile-kanban-scrolling.md` — backend sync, deployment, scoping, and
mobile board scrolling; none touch the issue panel's internal section
order.

## Consequence for spec/plan

Nothing in the knowledge base constrains or contradicts the planned
approach (move the render-prop block inside
`packages/ui/src/components/KanbanIssuePanel.tsx`, adjust the wrapper
border). Proceed as planned.
