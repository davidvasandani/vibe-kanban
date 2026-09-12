# Implementation Plan: Commits Behind in the Git Header

**Spec**: `./spec.md`
**Status**: Ready for task breakdown

## Technical Context

The change is in the React/TypeScript shared frontend under
`packages/web-core`, rendered for both local and remote web applications.
TanStack Query's existing `useBranchStatus` hook polls the workspace branch
status every 15 seconds and caches by workspace ID. `RightSidebar.tsx` owns the
Git collapsible header; `GitPanelContainer.tsx` owns the expanded body. The
shared `CollapsibleSectionHeader` already accepts a `headerExtra` node and
unmounts children while collapsed.

No backend, generated type, database, dependency, or deployment change is
needed.

## Architecture & Approach

1. Add `GitBehindHeader.tsx` beside `RightSidebar.tsx` as a web-core feature
   component. It accepts `workspaceId` and `RepoWithTargetBranch[]`, calls the
   existing `useBranchStatus`, and joins status to repository metadata by ID.
2. Keep pure derivation in an exported helper so cardinality, filtering,
   ordering, and copy can be tested without QueryClient setup. Preserve input
   repository order for stable output.
3. Return `null` when status is absent or no positive values exist. Render one
   bounded `text-low` span otherwise. Single-repository output is `<n> behind`;
   multi-repository output is `<display-name> <n>` joined with ` · `.
4. Supply verbose singular/plural title and accessible text for all values.
   The visible span uses `min-w-0`, a bounded maximum width, and `truncate`,
   matching existing dynamic header metadata.
5. Pass the component as the Git section's `headerExtra` in
   `RightSidebar.tsx`. Because header extras live outside section children, its
   query remains mounted when the Git body is collapsed. TanStack Query
   deduplicates the same `['branchStatus', workspaceId]` request while the body
   is expanded.
6. Extend `RightSidebar.test.tsx` to mock branch status and assert header
   placement/collapsed persistence, and add focused helper/component coverage
   for the presentation matrix.

## Data Model

See `./data-model.md`. Existing `RepoWithTargetBranch` and `RepoBranchStatus`
types are consumed unchanged.

## Contracts

See `./contracts/git-header-status.md`. No HTTP contract changes.

## Research Notes

See `./research.md`. No new dependency or external research is required.

## Constitution Check

- **I / III / VI:** the approach is a small, obvious extension that reuses the
  existing branch-status hook and header-extra seam.
- **II:** acceptance criteria are backed by focused pure/component and rendered
  sidebar tests.
- **IV:** fetching remains in `web-core`; the shared UI primitive is reused and
  does not gain feature-specific Git behavior.
- **XXI:** target resolution and divergence remain backend-owned; the frontend
  consumes the established `commits_behind` result without redefining it.
- **XIV:** verification will install the locked frontend dependencies first if
  the fresh worktree has not already been bootstrapped.

No constitution deviations or open questions remain.

## Risks & Dependencies

- The header and expanded body both subscribe to the branch-status query.
  TanStack Query deduplicates and shares this cache, but a regression test must
  guard collapsed availability rather than relying on body mounting.
- Very long repository names or many repositories can exceed drawer width.
  Bounded truncation plus complete title/accessible copy controls overflow.
- `commits_behind: null` is not a zero. Filtering must distinguish unavailable
  evidence from confirmed current status.
- The checked-in SpecKit command text contains a stale feature path from a prior
  task. Artifacts are deliberately written to the current branch-derived
  `specs/vk/a35b-commits-behind-m/` directory to avoid corrupting old records.
