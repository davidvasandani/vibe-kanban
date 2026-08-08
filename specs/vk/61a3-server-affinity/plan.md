# Implementation Plan: Server Affinity Sidebar Polish

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context

- React + TypeScript in `packages/web-core`, rendered by both local and remote
  frontend entrypoints.
- Tailwind utilities use the scoped design tokens documented in
  `packages/local-web/AGENTS.md` (`p-base`, `gap-half`, `text-low`, etc.).
- `RightSidebar.tsx` builds section definitions and passes `headerExtra` to the
  shared `packages/ui/src/components/CollapsibleSectionHeader.tsx` primitive.
- `ServerAffinitySectionContainer.tsx` owns placement queries and mutations.
- The task branch predates the merged affinity UI. `origin/main` is the required
  baseline before changing these files.

## Architecture & Approach

1. Fast-forward or merge the task branch to `origin/main`, retaining this task's
   pipeline documents. The canonical affinity UI already exists there and must
   be extended rather than recreated.
2. Extract the header-label precedence in
   `packages/web-core/src/pages/workspaces/RightSidebar.tsx` into a small pure
   helper if that provides a focused, low-mock test seam. The helper accepts the
   selected summary affinity and translations, and returns assigned hostname,
   requested hostname, or kind fallback.
3. Render the returned header label inside a `min-w-0`/bounded truncation wrapper
   passed through the existing `headerExtra` prop. Do not query affinity from the
   collapsed body.
4. Replace the affinity body's full-width `justify-between` rows in
   `ServerAffinitySectionContainer.tsx` with a compact two-column grid. Use a
   content-sized/stable label column and `minmax(0, 1fr)` control column; allow
   the select trigger to use the full value-column width with a reasonable max.
5. Preserve all query keys, mutation behavior, confirmation flow, worker
   eligibility, and localization.
6. Add focused Vitest coverage for the pure header-label decision and/or
   rendered sidebar behavior. Avoid pixel snapshots; assert visible content and
   stable overflow boundaries.
7. Run formatting and the narrowest relevant tests/checks, followed by the
   repository-required frontend verification.

## Data Model

No data-model changes. The feature consumes existing workspace summary affinity
fields: resolved `worker_hostname`, `requested_worker_hostname`, and `kind`.

## Contracts

No API changes. See `./contracts/sidebar-affinity.md` for the internal rendering
contract between workspace summary data, `RightSidebar`, and the shared
collapsible header.

## Research Notes

See `./research.md`.

## Constitution Check

- II (Test the contract): focused rendered/pure behavior coverage plus frontend
  checks.
- III and VI (Small steps; don't rebuild): reuse summary affinity and
  `headerExtra`; no new query, endpoint, or state.
- IV (Shared-component boundaries): keep feature data in `web-core`; use the
  existing `packages/ui` primitive without moving affinity logic into it.
- XXI (One convention per concept): retain the established affinity label
  precedence.
- XXII (Collapsed controls retain decisive context): keep summary-backed server
  context in the header with explicit truncation.

No deviations are planned.

## Risks & Dependencies

- Baseline reconciliation may conflict with root pipeline documents because
  earlier features also rewrite them; resolve in favor of this task's docs.
- A header child can have `truncate` yet still overflow if its parent flex item
  lacks `min-w-0`; verification must inspect both boundaries.
- Tests that mount the entire sidebar could require many provider mocks. Prefer
  a small pure label helper unless an existing sidebar harness is available.
- Changing `CollapsibleSectionHeader` globally would affect many surfaces; avoid
  it unless inspection proves the shared flex boundary itself is defective.
