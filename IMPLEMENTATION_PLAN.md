# Implementation Plan: Right Drawer Expand to Available Space

1. Inspect the shared `CollapsibleSectionHeader` contract and the workspace
   `RightSidebar` composition to identify which element owns expansion state
   and which element participates in flex sizing.
2. Extend `CollapsibleSectionHeader` with a narrowly scoped opt-in layout mode
   that makes its root grow and shrink while expanded, but remain intrinsically
   sized while collapsed. Preserve current behavior for every existing caller.
3. Convert the right drawer and section stack into a bounded full-height flex
   column, remove the per-section viewport-derived maximum height, and opt its
   sections into the new flexible expansion mode.
4. Keep the section body as the overflow owner and add the minimum-height
   constraints required for nested flex scrolling.
5. Add focused component tests proving the opt-in root classes change correctly
   across expanded and collapsed states and that legacy callers retain their
   default layout behavior.
6. Run formatting, focused tests, frontend type checking, and relevant lint.
7. Run the required independent Codex diff review; address confirmed findings
   and repeat verification until no significant findings remain.
8. Record any reusable right-drawer/flex-layout knowledge in the Vibe Kanban
   wiki, add this task id to the page, refresh its index, and commit the
   knowledge-base update separately before handoff.

## Expected files

- `packages/ui/src/components/CollapsibleSectionHeader.tsx`
- `packages/ui/src/components/CollapsibleSectionHeader.test.tsx` (new)
- `packages/web-core/src/pages/workspaces/RightSidebar.tsx`
- `wiki/right-drawer-flexible-sections.md` (if implementation confirms reusable
  guidance)
- `wiki/INDEX.md` (if a knowledge page is added)

## Verification

- Focused Vitest component test for `CollapsibleSectionHeader`.
- Relevant workspace/package TypeScript check.
- Relevant ESLint invocation or repository lint command.
- `pnpm run format` as required by repository guidance.
- Independent Codex review of the final implementation diff.
