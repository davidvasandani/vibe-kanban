# Tasks: Filter sidebar workspaces by issue status

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes.

## Phase 1: Setup
- [x] T001 [P] Add `hidden_issue_status_names: Vec<String>` (`#[serde(default)]`) to `WorkspaceFilterStateData`, plus unit tests for the legacy-payload default and the round trip, in `crates/db/src/models/scratch.rs`
- [x] T002 [P] Add pure `normalizeStatusName`, `filterSidebarWorkspaces` and `buildIssueStatusFilterOptions`, and the `NO_PROJECT_FILTER` / `NO_ISSUE_STATUS_FILTER` constants, in `packages/web-core/src/pages/workspaces/workspaceSidebarFilters.ts`
- [x] T003 [P] Add `useProjectsIssueStatuses(projectIds, enabled)` in `packages/web-core/src/pages/workspaces/useProjectsIssueStatuses.ts`
- [x] T004 [P] Add i18n keys `issueStatusFilterLabel`, `issueStatusFilterMenuLabel` and `noIssue`, and update `filterDialogDescription`, in `packages/web-core/src/i18n/locales/{en,es,fr,ja,ko,zh-Hans,zh-Hant}/common.json`

## Phase 2: Core
- [x] T005 Regenerate `shared/types.ts` with `pnpm run generate-types` (depends on T001)
- [x] T006 Add `hiddenIssueStatusNames` to `WorkspaceFilterState`, its default and the `setWorkspaceHiddenIssueStatusFilter` action in `packages/web-core/src/shared/stores/useUiPreferencesStore.ts` (depends on T005)
- [x] T007 Map `hidden_issue_status_names` ↔ `hiddenIssueStatusNames` in `packages/web-core/src/shared/hooks/useUiPreferencesScratch.ts` (depends on T005, T006)
- [x] T008 Wire everything into `packages/web-core/src/pages/workspaces/WorkspacesSidebarContainer.tsx`: build `remoteByLocalId`, gate the hook, replace the duplicated filter memos with `filterSidebarWorkspaces`, add the dialog's status `MultiSelectDropdown`, and extend `hasActiveFilters` (depends on T002, T003, T004, T006)

## Phase 3: Validation
- [x] T009 [P] Unit tests for filter and options (fail open, case-insensitive cross-project, No issue, AND composition, stale hidden names, input immutability) in `packages/web-core/src/pages/workspaces/workspaceSidebarFilters.test.ts` (depends on T002)
- [x] T010 Run the verification: `pnpm run generate-types:check`, `cargo test -p db`, vitest, `pnpm run check`, `pnpm run lint`, `scripts/check-i18n.sh` and `pnpm run format` (depends on T001–T009)
- [ ] T011 Runtime check that the filter dialog renders the status control and hides matching workspaces (depends on T010) — **Not completed:** the dev stack built and served on this worker (think3), but no browser could reach it. `agent-browser` is not installed, the workspace browser runs on another host, and the preview lease could not be scoped to this workspace. Verify on deploy.
