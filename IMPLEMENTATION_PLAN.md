# Implementation plan — vk/4dac-filter-workspace

Spec: `SPEC.md`. Prior knowledge: `PRIOR_KNOWLEDGE.md`.

## 1. Persisted preference (Rust + generated types)

- `crates/db/src/models/scratch.rs`: add
  `#[serde(default)] pub hidden_issue_status_names: Vec<String>` to
  `WorkspaceFilterStateData`.
- Add a unit test: an older payload without the field deserializes with an
  empty vec, and a payload with the field round-trips.
- `pnpm run generate-types` to update `shared/types.ts`. Never hand-edit it.

## 2. Store + scratch sync (web-core)

- `useUiPreferencesStore.ts`: add `hiddenIssueStatusNames: string[]` to
  `WorkspaceFilterState` and its default, plus the
  `setWorkspaceHiddenIssueStatusFilter(names)` action. `clearWorkspaceFilters`
  already resets to the default.
- `useUiPreferencesScratch.ts`: map `hidden_issue_status_names` in
  `storeToScratchData` and `scratchDataToStore`, with a `?? []` fallback.

## 3. Pure logic (new `pages/workspaces/workspaceSidebarFilters.ts`)

- `normalizeStatusName(name)`: trims and lower-cases.
- `NO_ISSUE_STATUS_FILTER = '__no_issue__'`.
- `buildIssueStatusFilterOptions(statuses, hiddenNames)`: dedupes by
  normalized name, keeps the first-seen display label, orders by lowest
  `sort_order` then label, and appends hidden names that are missing from the
  data. The UI prepends a **No issue** option.
- `filterSidebarWorkspaces(workspaces, criteria)`: one function for both the
  active and the archived list. It applies project, PR, issue status and
  search, and replaces the two duplicated `useMemo` blocks in
  `WorkspacesSidebarContainer`. The criteria come in as plain lookups:
  `remoteByLocalId: Map<localId, {projectId, issueId}>` and
  `issueStatusNameById: Map<issueId, normalizedName>`.
- Tests in `workspaceSidebarFilters.test.ts`: hide by name across projects,
  case-insensitivity, fail open (no remote record, issue not loaded, status
  unknown), **No issue** (both `issue_id = null` and no remote record), AND
  composition with the project, PR and search filters, and options
  dedupe / order / stale hidden names. Pair each absence assertion with a
  positive case.

## 4. Data hook (new `pages/workspaces/useProjectsIssueStatuses.ts`)

- Input: `projectIds: string[]` and `enabled`. For each project it subscribes
  to `PROJECT_ISSUES_SHAPE` and `PROJECT_PROJECT_STATUSES_SHAPE` through
  `createShapeCollection` + `subscribeChanges`, the same way
  `useAllOrganizationProjects` does.
- Output: `statuses: ProjectStatus[]` and
  `issueStatusNameById: Map<issueId, normalizedName>`.
- Keys the effect by a sorted, joined project-id string, so identity churn
  doesn't resubscribe.

## 5. UI (`WorkspacesSidebarContainer.tsx`)

- Build `remoteByLocalId` from `useUserContext().workspaces`, replacing
  `remoteProjectByLocalId`. The project-group computation still derives
  project ids from it.
- `statusDataEnabled = isFilterDialogOpen || hiddenIssueStatusNames.length > 0`.
- Linked project ids come from the remote workspaces.
- `WorkspacesFilterDialog`: add a `MultiSelectDropdown` (label "Hide issue
  status", menu label "Hide workspaces whose issue is…", `EyeSlashIcon`). The
  selected values are the normalized names being hidden.
- `hasActiveFilters` includes `hiddenIssueStatusNames.length > 0`.
- Update the dialog description.

## 6. i18n

- New keys under `kanban.workspaceSidebar`: `issueStatusFilterLabel`,
  `issueStatusFilterMenuLabel`, `noIssue`. Update `filterDialogDescription`.
  Add them to all 7 locales.

## 7. Verify

- `pnpm install --frozen-lockfile`, `pnpm run generate-types:check`,
  `cargo test -p db`, the web-core vitest for the new test,
  `pnpm run check`, `pnpm run lint`, `scripts/check-i18n.sh`,
  `pnpm run format`.
- Runtime check: run the app if feasible and confirm the dialog renders.
