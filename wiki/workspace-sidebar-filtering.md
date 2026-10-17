# Workspace sidebar filtering

How the workspaces sidebar (`WorkspacesSidebarContainer.tsx`) decides which
workspaces it lists. Also covers the reusable rules behind filtering local
workspaces by **remote** facts, such as the linked issue's status.

## One pure pipeline for both lists

Every sidebar filter (project, PR, issue status, search) runs through the pure
`filterSidebarWorkspaces` in
`packages/web-core/src/pages/workspaces/workspaceSidebarFilters.ts`. The
container calls it once for the active list and once for the archived list,
with the same criteria object. Sorting (`workspaceSort.ts`) and pagination
come after it. Before, the active and archived lists each had a hand-copied
filter block, and every new filter had to be added twice. Add new filters to
the pure function and cover them in `workspaceSidebarFilters.test.ts` (node
Vitest, plain fixtures).

## Joining local workspaces to remote facts

A sidebar row is a *local* workspace. Its project and issue come from the
user-scoped remote workspace shape (`useUserContext().workspaces`), joined
through `local_workspace_id` into
`remoteByLocalId: Map<localId, {projectId, issueId}>`. An issue's status needs
a second hop, `issue.status_id` → `ProjectStatus.name`. That data lives in the
per-project `PROJECT_ISSUES_SHAPE` / `PROJECT_PROJECT_STATUSES_SHAPE`, which
`useProjectsIssueStatuses` subscribes to through `createShapeCollection` +
`subscribeChanges`, in the same way as `useAllOrganizationProjects`. It does
this because the number of projects varies, and `useShape` can't be called in
a loop.

Two rules come from this:

- **Gate expensive enrichment on need.** Issue shapes are whole-project
  downloads. The hook is enabled only while the filter dialog is open or a
  status is hidden, so the default sidebar opens no new subscriptions. Key the
  subscription effect on a sorted, joined id string, not on the array,
  because the remote workspace list changes identity on every Electric
  update.
- **Fail open (constitution XXXIV).** Hide a workspace only when its status is
  known *and* hidden. A workspace with no remote row, or whose issue or status
  hasn't loaded, stays visible. The one deliberate exception is the
  `__no_issue__` sentinel. Being "not linked to an issue" doesn't depend on
  loading, so it hides both `issue_id = null` rows and workspaces with no
  remote row. That mirrors the existing `__no_project__`.

## Global preferences over per-project statuses

Status ids are per project, and statuses have no category column. A global
preference therefore stores **normalized names** (`trim().toLowerCase()`),
the same convention as `list_view_status_filter_name`. It stores a **hide
list**, so a new status or project shows by default rather than silently
hiding work. The options list keeps any hidden name that no loaded project
has, so a renamed or deleted status can still be un-hidden.

## Gotcha: the preference scratch is typed on the server

The UI-preferences scratch round-trips through `UiPreferencesData` in
`crates/db/src/models/scratch.rs`. Serde has no catch-all there, so a field
added only on the frontend is **silently dropped** on save and disappears
after a reload. For a new preference:

1. Add the Rust field with `#[serde(default)]`, plus a test that the legacy
   payload still loads.
2. Run `pnpm run generate-types`.
3. Map the field in both `storeToScratchData` and `scratchDataToStore`
   (`useUiPreferencesScratch.ts`).

## Verification notes

- `pnpm run lint` covers `packages/web-core` (and `packages/remote-web`) with
  local-web's rules (see [frontend-linting](frontend-linting.md)). To lint
  only web-core, run `pnpm run web-core:lint`, or `npx eslint src/<file>`
  from `packages/web-core`. The old `--no-eslintrc` workaround is gone.
- `scripts/check-i18n.sh`'s duplicate-key check needs GNU `diff`. On worker
  hosts that lack diffutils it reports "duplicate keys detected" for **every**
  locale file, including untouched ones. That is an environment failure, not a
  real duplicate. The key-consistency and unused-key checks still run
  correctly.

## Contributed by

- vk/4dac-filter-workspace
- vk/848f-lint-packages-we (verification notes)
