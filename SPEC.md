# Technical Spec: Move Issue Workspace Box above Title and Description

> Task b37f. Full SpecKit artifacts live in
> `homelab/specs/vk/b37f-move-issue-works/` (`spec.md`, `plan.md`,
> `tasks.md`). This file is the repo-root technical summary.

## Problem

In the kanban issue detail panel, the Workspaces box (linked
workspaces/attempts plus the create/link actions) renders below the issue
title, description, and the create-mode blocks. Users work workspace-first:
opening an in-progress issue means scrolling past an often-long description
to reach the section they act on most.

## Solution

Move the edit-mode Workspaces section above the title/description block, so
the panel reads: property row → tags row → **Workspaces box** → title →
description → SpecKit → Relationships → Sub-issues → Comments. Positional
change only — same component, same data, same actions.

## Where it lives

Single presentational component shared by local-web and remote-web:

- `packages/ui/src/components/KanbanIssuePanel.tsx` — the edit-mode block
  `{!isCreateMode && issueId && renderWorkspacesSection && (...)}` moves
  from after the create-button block to immediately before the
  "Title and Description" container. The moved wrapper's class changes
  `border-t` → `border-b`: the tags row above already draws `border-b`, so
  keeping `border-t` would double the separator; below, the next remaining
  section (SpecKit/Relationships) still draws its own top border against the
  description, so no separator is lost.

The container (`packages/web-core/src/pages/kanban/KanbanIssuePanelContainer.tsx`)
and the section itself (`IssueWorkspacesSectionContainer.tsx`,
`packages/ui/src/components/IssueWorkspacesSection.tsx`) are untouched —
the `renderWorkspacesSection` render-prop wiring is unchanged.

## Behavior invariants

- Create mode is unchanged (the Workspaces box never renders there; the
  `!isCreateMode` guard is preserved).
- Workspaces box content/actions unchanged (create, link, open, unlink,
  archive, delete).
- All other sections keep their relative order.

## Validation

- `packages/remote-web/src/test/KanbanIssuePanel.test.tsx` — rendered-DOM
  test asserting the Workspaces section precedes title and description in
  edit mode (and trailing sections still follow the description), and that
  the section is absent in create mode. Confirmed to fail against the
  pre-change layout.
- Gates: `pnpm run check`, `pnpm run lint`, `cargo test --workspace`,
  `pnpm run format` — all green.
