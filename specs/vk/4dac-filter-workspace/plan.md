# Implementation Plan: Filter sidebar workspaces by issue status

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context
- Frontend: React + TypeScript in `packages/web-core` (shared by local-web and
  remote-web), primitives from `packages/ui` (`@vibe/ui`). Tests use Vitest
  (node env for pure logic).
- Remote data: ElectricSQL shapes via `createShapeCollection`
  (`packages/web-core/src/shared/lib/electric/collections.ts`), with a REST
  fallback. The shapes are `PROJECT_ISSUES_SHAPE` and
  `PROJECT_PROJECT_STATUSES_SHAPE` (`shared/remote-types.ts`).
- Preference storage: the UI-preferences scratch, a typed Rust struct
  `UiPreferencesData` in `crates/db/src/models/scratch.rs`, stored as JSON in
  SQLite and exported to `shared/types.ts` by ts-rs
  (`crates/server/src/bin/generate_types.rs`).
- Constraint: generated types are never hand-edited (constitution
  Constraints).

## Architecture & Approach
Data flow for one sidebar render:

```
useWorkspaceContext()  → local active / archived SidebarWorkspace[]
useUserContext()       → remote Workspace[] (local_workspace_id, project_id, issue_id)
        │ remoteByLocalId: Map<localId, {projectId, issueId}>
        ▼
useProjectsIssueStatuses(linkedProjectIds, enabled)
        → statuses: ProjectStatus[], issueStatusNameById: Map<issueId, normalizedName>
        ▼
filterSidebarWorkspaces(list, criteria)   (pure; same call for active + archived)
        ▼
sortWorkspaces → paginate → <WorkspacesSidebar/>
```

FR mapping:

| FR | Where |
|----|-------|
| FR-1, FR-7 | `buildIssueStatusFilterOptions` in the new `pages/workspaces/workspaceSidebarFilters.ts`. The `MultiSelectDropdown` goes in `WorkspacesFilterDialog` (`WorkspacesSidebarContainer.tsx`). |
| FR-2, FR-3, FR-5, FR-6, FR-8 | `filterSidebarWorkspaces` (pure). It replaces the two duplicated `useMemo` filter blocks in `WorkspacesSidebarContainer.tsx`. |
| FR-4, FR-10 | `WorkspaceFilterStateData.hidden_issue_status_names` (Rust, `#[serde(default)]`), `WorkspaceFilterState.hiddenIssueStatusNames` (`useUiPreferencesStore.ts`), and the mapping both ways in `useUiPreferencesScratch.ts`. |
| FR-9 | `hasActiveFilters` in `WorkspacesSidebarContainer.tsx`. `clearWorkspaceFilters` already resets to `DEFAULT_WORKSPACE_FILTER_STATE`. |
| FR-11 | `useProjectsIssueStatuses` is enabled only when `isFilterDialogOpen || hiddenIssueStatusNames.length > 0`. |
| FR-12 | No change to `packages/ui/src/components/WorkspacesSidebar.tsx`. It already groups whatever list it receives. |
| FR-13 | `filterSidebarWorkspaces` has no attention exception. |

New hook `packages/web-core/src/pages/workspaces/useProjectsIssueStatuses.ts`
follows `useAllOrganizationProjects.ts`. For each project it calls
`createShapeCollection` + `subscribeChanges({ includeInitialState: true })`
on both shapes, aggregates into state, and unsubscribes on cleanup. It keys
the effect by a sorted, joined project-id string. It does not use `useShape`,
because that would mean calling a hook in a loop.

The dialog selects values by normalized name (`trim().toLowerCase()`). The
option label is the first-seen display name. For a stale hidden name the label
is the stored string itself.

## Data Model
See `./data-model.md`.

## Contracts
See `./contracts/workspace-sidebar-filters.ts` (pure-function signatures) and
`./contracts/ui-preferences-scratch.md` (persisted field).

## Research Notes
See `./research.md`. No new dependencies.

## Constitution Check
- **I Clarity / III Small steps:** reuses the existing dialog,
  `MultiSelectDropdown`, preference scratch and cached shapes. Merging the
  duplicated active/archived filter blocks into one pure function removes
  duplication rather than adding it.
- **II Test the contract:** Vitest unit tests for the pure filter and the
  options builder, plus a Rust unit test for the scratch field's default and
  round trip.
- **IV Shared-component boundaries:** only an existing `@vibe/ui` primitive
  is used. The logic stays in web-core. Blast radius: local-web and
  remote-web both render `WorkspacesSidebarContainer`. That is intended.
- **XXXIV Partial projections degrade deterministically:** fail open when the
  issue or status is unknown. Tested in the base-only state (no status data)
  and the enriched state.
- **XXXV Matching identity:** status comes only from the workspace's own
  remote row, `local_workspace_id` → `issue_id` → `status_id` → name. Nothing
  is borrowed from siblings.
- **XXXVIII:** this is a deliberate filter, so it keeps applying during
  search, like the project and PR filters.
- **Constraints:** `shared/types.ts` is regenerated with
  `pnpm run generate-types`. `pnpm run format` runs before completion.
No deviations.

## Risks & Dependencies
- **Sync cost:** subscribing to every linked project's issues shape can be
  large for big projects. This is mitigated by the enable gate (FR-11) and by
  the collection cache, which the kanban board and `LinkedIssueProvider`
  already share.
- **Renamed statuses:** a hidden name that no project uses any more stops
  matching, but stays listed so it can be cleared (FR-7). This is accepted.
- **Rust type drift:** if the Rust field were missing, the server would drop
  the preference silently. That is covered by the round-trip test and by
  `generate-types:check`.
