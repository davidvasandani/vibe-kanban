# Tasks: Move deployment refresh

**Plan**: `./plan.md`

Tasks are dependency ordered. Tasks marked **[P]** touch independent files and
may be completed together within their phase.

## Phase 1: Shared UI contracts

- [x] T001 Add explicit labeling support for isolated section-header actions in
      `packages/ui/src/components/CollapsibleSectionHeader.tsx`.
- [x] T002 [P] Remove web deployment Refresh/revision rendering while retaining
      native Update behavior in `packages/ui/src/components/AppBar.tsx`.
- [x] T003 [P] Add the Deploy Status accordion persist key and union member in
      `packages/web-core/src/shared/stores/useUiPreferencesStore.ts`.

## Phase 2: Feature wiring

- [x] T004 Convert the fixed Deploy Status row into the first intrinsic
      collapsible section, add conditional Refresh action props, and retain
      status metadata in the header in
      `packages/web-core/src/pages/workspaces/RightSidebar.tsx` (depends on T001,
      T003).
- [x] T005 Consume existing deployment-update availability and pass the refresh
      callback into the sidebar in
      `packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx` (depends on
      T004).

## Phase 3: Validation

- [x] T006 [P] Update rendered-DOM coverage for Deploy Status accordion
      placement, metadata, conditional Refresh invocation, and disclosure
      isolation in `packages/web-core/src/pages/workspaces/RightSidebar.test.tsx`
      (depends on T004).
- [x] T007 [P] Add AppBar rendered-DOM coverage for removed revision/Refresh and
      retained native Update behavior in
      `packages/web-core/src/shared/components/ui-new/AppBar.test.tsx` (depends
      on T002).
- [x] T008 Run locked dependency setup, focused tests, frontend checks, lint,
      formatting, and diff validation; fix failures (depends on T005-T007).

## Phase 4: Review and delivery

- [x] T009 Run independent Codex diff review, address confirmed findings, and
      repeat verification/review to no significant findings (depends on T008).
- [x] T010 Record reusable deployment-control ownership guidance in
      `docs/knowledge-base/responsive-deployment-identity.md` and refresh
      `docs/knowledge-base/INDEX.md`, or explicitly record that no new knowledge
      emerged; commit the knowledge-base result (depends on T009).
- [ ] T011 Commit remaining implementation, push the branch, open a PR against
      the base branch, wait for required checks, and merge (depends on T010).

## Dependency graph

- Layer A (parallel): T001, T002, T003
- Layer B: T004
- Layer C (parallel after T004): T005, T006, T007
- Layer D: T008 → T009 → T010 → T011
