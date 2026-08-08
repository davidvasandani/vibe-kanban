# Implementation Plan: Server Affinity Sidebar Polish (`61a3`)

1. Reconcile the task branch with the current `origin/main` baseline, which
   already contains the Server Affinity UI and its summary-backed header data.
   Preserve all task documentation and resolve only task-relevant conflicts.
2. Inspect `CollapsibleSectionHeader`, `RightSidebar`, and
   `ServerAffinitySectionContainer` at the reconciled baseline to confirm the
   available header slot, disclosure spacing, sidebar width constraints, and
   existing affinity label precedence.
3. Refactor the expanded affinity body into a compact grid/two-column layout
   using design-system spacing tokens. Give the label column a stable compact
   width and the value/control column the remaining width, with safe `min-w-0`
   and truncation behavior.
4. Ensure the collapsed section header renders the current server label from
   the selected workspace summary using assigned hostname, requested hostname,
   and translated kind in that order. Constrain it so long names do not overlap
   actions or the disclosure affordance.
5. Add focused regression coverage for header-label selection/visibility and,
   where practical, stable semantic layout classes without pixel snapshots.
6. Run the focused frontend tests, formatter, TypeScript/frontend checks, and
   lint relevant to the changed packages. Inspect the resulting diff for scope
   and generated/unrelated changes.
7. Run the independent Codex review loop, address confirmed significant
   findings, and repeat verification until the review is clean.
8. Distill only genuinely reusable UI knowledge into the project knowledge
   base, tag it with `61a3`, refresh the index, and commit that knowledge-base
   update separately before handoff.

## Dependency and parallelism notes

- Steps 1–2 are prerequisites for all implementation work.
- Body-spacing work and collapsed-header coverage can be implemented together
  after the baseline inspection because they touch closely related code and
  must be reviewed as one responsive layout.
- Formatting and verification follow implementation; independent review follows
  a green local verification pass; knowledge distillation follows the final
  reviewed behavior.

## Expected files

- `packages/web-core/src/pages/workspaces/RightSidebar.tsx`
- `packages/web-core/src/pages/workspaces/ServerAffinitySectionContainer.tsx`
- Focused tests colocated with the relevant workspace sidebar code, if absent
  test seams can be introduced without widening scope
- Pipeline documents under `specs/vk/61a3-server-affinity/`
- `docs/knowledge-base/INDEX.md` and one relevant topic page only if reusable
  knowledge emerges
