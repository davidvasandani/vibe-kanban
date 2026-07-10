# Implementation Plan: Move Issue Workspace Box above Title and Description (task vk/b37f-move-issue-works)

Step-by-step build order. The authoritative dependency-ordered task list is
`homelab/specs/vk/b37f-move-issue-works/tasks.md` (T001–T003); this is the
executable narrative. Rationale in `SPEC.md`; prior-art recall in
`PRIOR_KNOWLEDGE.md`.

## Step 1 — Move the block (T001)

In `packages/ui/src/components/KanbanIssuePanel.tsx`, inside the scrollable
content:

1. Cut the edit-mode Workspaces block
   `{!isCreateMode && issueId && renderWorkspacesSection && (<div>…</div>)}`
   from its position after the create-button block.
2. Paste it immediately before the `{/* Title and Description */}` container
   (after the tags row).
3. Change the wrapper class `border-t` → `border-b` so exactly one separator
   renders at each boundary (tags row already draws `border-b` above; the
   SpecKit/Relationships sections below the description draw their own top
   borders).

No changes to `KanbanIssuePanelContainer.tsx` or the workspaces section
components — the render-prop contract is untouched.

## Step 2 — Verify rendered order (T002)

- Launch `pnpm run dev`; confirm app serves. (In the headless task
  environment the kanban board sits behind GitHub sign-in, so DOM-order
  verification is done with a component test instead.)
- Add `packages/remote-web/src/test/KanbanIssuePanel.test.tsx` (jsdom +
  testing-library, same harness as the existing `@vibe/ui` component tests
  there): edit mode → workspaces section precedes title and description,
  comments still follow description; create mode → no workspaces section.
- Prove the test bites: stash the Step-1 edit, test fails; unstash, passes.

## Step 3 — Gates (T003)

From the `vibe-kanban` repo root: `pnpm run check`, `pnpm run lint`,
`cargo test --workspace`, `pnpm run format`. All must pass.

## Status

All steps executed; all gates green.
