# Implementation Plan: Stable Workspace Order During Restart

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context

The affected surface is the React/TypeScript workspace sidebar in
`packages/web-core`. `useWorkspaces.ts` combines streamed `WorkspaceWithStatus`
records with `WorkspaceSummary` records fetched through TanStack Query.
`WorkspacesSidebarContainer.tsx` then filters, sorts, and paginates the resulting
`SidebarWorkspace` objects. The base record's `updated_at` is already mapped to
`updatedAt`; summary activity is mapped to `latestProcessCompletedAt`.

## Architecture & Approach

1. Extract the timestamp-selection and comparison policy from
   `WorkspacesSidebarContainer.tsx` into a small sibling module so it can be
   tested without rendering the full sidebar and its provider graph.
2. For `updated_at`, select the first valid timestamp from
   `latestProcessCompletedAt` and `updatedAt`. For `created_at`, select the valid
   `createdAt` value.
3. Compare pinning before timestamps, keep missing/invalid selected times after
   valid ones in either direction, apply the chosen direction only to two valid
   times, then compare name and ID for stable ties.
4. Keep the existing `useCallback`/`useMemo` filtering, sorting, pagination, and
   active/archive flow; replace only the inline comparator call.
5. Add a focused Vitest suite beside the helper, using minimal
   `SidebarWorkspace` fixtures for base-only startup and enriched states.

## Data Model

See `./data-model.md`. No persisted model or generated type changes are needed.

## Contracts

No HTTP, WebSocket, database, or generated-type contract changes are needed.
The change is confined to projection logic over existing fields.

## Research Notes

See `./research.md`. No new dependency is introduced.

## Constitution Check

- Principle II: focused tests cover the explicit ordering contract.
- Principle III: the change reuses existing streamed and summary timestamps and
  adds no data plumbing.
- Principle IV: data/projection logic remains in `packages/web-core`; no
  presentational primitive is duplicated.
- Principle VI: the existing sidebar sort path is extended rather than forked.
- Principle XXXI: base-only and enriched states have deterministic behavior,
  and missing enrichment cannot outrank valid timestamps.
- Constraints: generated types remain untouched and repository formatting will
  run before completion.

No constitution deviations or open questions remain.

## Risks & Dependencies

- Base `updated_at` may move for workspace mutations unrelated to process
  completion. It is lower priority than a valid process-completion time and is
  preferable to alphabetical startup ordering.
- Richer summary arrival can still refine list order; this is truthful behavior,
  while the initial state is immediately useful.
- Tests depend on the repository's existing Vitest setup and require the locked
  pnpm dependency installation in a fresh worktree.
