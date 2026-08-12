# Tasks: Discoverable mobile workspace right drawer

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may be performed together within their phase.

## Phase 1: Contract tests

- [x] T001 Add failing rendered-DOM coverage for the mobile right-sidebar tab's
  accessible name, selected state, icon metadata, and activation callback in
  `packages/web-core/src/shared/components/ui-new/Navbar.mobile.test.tsx`.
- [x] T002 [P] Add failing pure coverage for workspace-dependent mobile-tab
  availability in
  `packages/web-core/src/shared/components/ui-new/containers/workspaceMobileTabs.test.ts`.

## Phase 2: Shared presentation

- [x] T003 Extend mobile-tab metadata and render semantics, and change the
  existing `git` destination to the right-sidebar presentation in
  `packages/ui/src/components/Navbar.tsx` (depends on T001).

## Phase 3: Workspace availability wiring

- [x] T004 Implement the pure workspace-tab availability selector in
  `packages/web-core/src/shared/components/ui-new/containers/workspaceMobileTabs.ts`
  (depends on T002, T003).
- [x] T005 Pass workspace-aware mobile tabs from
  `packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx`
  (depends on T004).
- [x] T006 Recover from an unavailable active `git` tab when entering create
  mode or the workspace-less landing in
  `packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx`, with focused
  regression coverage (depends on T004).

## Phase 4: Verification

- [x] T007 Run the focused web-core Vitest files and correct regressions
  (depends on T003, T005, T006).
- [x] T008 [P] Run `pnpm --filter @vibe/ui check` and
  `pnpm --filter @vibe/web-core check` (depends on T003, T005, T006).
- [x] T009 [P] Run the relevant frontend lint commands and inspect the narrow
  mobile tab strip behavior (depends on T003, T005, T006).
- [x] T010 Run `pnpm run format`, re-run focused tests and type checks, and
  inspect the final diff for scope (depends on T007, T008, T009).

## Phase 5: Delivery

- [x] T011 Run independent Codex review, resolve all significant confirmed
  findings, and re-verify (depends on T010).
- [x] T012 Update `wiki/` and `wiki/INDEX.md` with reusable mobile drawer/tab
  knowledge, tag it with this task id, and commit the knowledge base (depends on
  T011).
- [x] T013 Open a pull request against the base branch and merge it after checks
  pass (depends on T012).
