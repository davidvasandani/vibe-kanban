# Prior Knowledge — recalled for `vk/77eb-vk-pipeline`

Searched the project knowledge base — `wiki/` (8 topic pages + INDEX) — for
pages relevant to this task (adding the Pipeline control to the issue
detail panel in edit mode, with an "Update Issue" apply button;
`packages/ui` + `packages/web-core`). One page covers the exact component;
one establishes the governing frontend convention.

## Relevant findings

**[wiki/kanban-issue-panel-sections.md] — directly on point.** The issue
panel (`packages/ui/src/components/KanbanIssuePanel.tsx`) owns section
order; containers only supply render props (`renderPipeline` is already one
of them). Consequences for this task:

- Opening the pipeline slot in edit mode is a change in the `packages/ui`
  panel (the `isCreateMode &&` gate), not in the container's ordering. The
  panel is shared by local-web and remote-web, so one edit covers both.
- Border convention: one separator per boundary. The pipeline card renders
  its own `border-t` and sits after the description block, directly above
  sections that draw their own top borders — keeping it in the same slot in
  edit mode needs no border flip.
- Edit-mode sections use the full guard `!isCreateMode && issueId &&
  renderXxx` — but for pipeline the container itself branches on mode, so
  the panel-side gate simply becomes `renderPipeline && renderPipeline()`
  (container returns per-mode content or null).
- Testing recipe for panel-order/rendering tests lives in
  `packages/remote-web/src/test/*.test.tsx`; run via `pnpm test`
  (`NODE_ENV=test` gotcha), match testids/keys not translated strings.

**[wiki/appbar-rail-and-org-tiles.md] — convention + review gotchas.**
Reinforces the container/presentational split. Two Codex-review gotchas
that apply here: (1) don't render a no-op interactive element — the
edit-mode "Update Issue" button should be disabled when the pipeline
selection isn't dirty rather than silently doing nothing; (2) optional
props on shared components need working fallbacks — the new
`PipelineSection` props (`initialBlock`, `seedDefaultPipeline`, `footer`)
must default to today's create-mode behavior so existing callers are
untouched.

**[wiki/electric-sync-fallback.md] — persistence context.** Issue updates
go through the ElectricSQL optimistic-write mutation layer
(`updateIssue` from `useProjectContext`), same as the debounced
title/description saves. The pipeline apply should reuse that path — no
new API surface, and no special error handling beyond what description
saves already do.

## Not relevant

`kanban-items-state-and-activity-grouping.md` (board state, not panel
content), `mobile-kanban-scrolling.md`, `self-hosted-deployment.md`,
`project-context-map.md`, `external-connector-sync.md` — board scrolling,
deployment, scoping, and backend connector sync; none touch pipeline
composition or the issue panel's edit mode.

## Consequence for spec/plan

The knowledge base confirms the planned shape: open the existing
`renderPipeline` slot in the `packages/ui` panel, keep all behavior in the
web-core container/section, persist via the existing `updateIssue`
mutation, and make the "Update Issue" button disabled-when-clean. Nothing
contradicts the SPEC.md approach.
