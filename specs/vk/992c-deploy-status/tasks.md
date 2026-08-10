# Tasks: Desktop Deploy Status

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their layer. Each task names the files it changes.

## Phase 1: Shared presentation

- [x] T001 Add an optional desktop/full-age presentation mode while preserving
  the current mobile default in
  `packages/ui/src/components/DeployStatus.tsx`.
- [x] T002 [P] Extend deterministic shared deploy-status coverage for the
  desktop presentation mode and retain production, `dev`, missing, malformed,
  and timer behavior in
  `packages/remote-web/src/app/layout/Navbar.test.tsx` (depends on T001).

## Phase 2: Desktop drawer integration

- [x] T003 [P] Consume existing user-system deployment metadata and render a fixed,
  intrinsic-height `Deploy Status` row before all collapsible sections in
  `packages/web-core/src/pages/workspaces/RightSidebar.tsx`, enabled only by the
  desktop mount in `packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx`
  (depends on T001).
- [x] T004 Add rendered-DOM regression coverage for row placement,
  non-collapsibility, metadata propagation, and drawer sizing classes in
  `packages/web-core/src/pages/workspaces/RightSidebar.test.tsx` (depends on
  T003).

## Phase 3: Validation

- [x] T005 Run locked dependency setup if needed, focused UI/web-core tests,
  shared frontend type checks, `pnpm run generate-types:check`, lint, formatting,
  and `git diff --check`; record results in
  `specs/vk/992c-deploy-status/verification.md` (depends on T002, T004).
- [x] T006 Cross-check the landed implementation against `spec.md`, `plan.md`,
  and this task list, then tick completed items in
  `specs/vk/992c-deploy-status/tasks.md` (depends on T005).

## Phase 4: Review and finish

- [x] T007 Run independent Codex CLI review, address confirmed findings, repeat
  verification as needed, and record the clean result in
  `specs/vk/992c-deploy-status/review.md` (depends on T006).
- [x] T008 Update the relevant right-drawer/deployment topic in `wiki/`, tag it
  with `VAS-377`, refresh `wiki/INDEX.md` if needed, and commit the knowledge-base
  change; record the result in `specs/vk/992c-deploy-status/knowledge.md`
  (depends on T007).
- [ ] T009 Merge branch `vk/992c-deploy-status` into its configured base branch
  after confirming the repository is clean and the diff is limited to Vibe
  Kanban (depends on T008).

## Dependency graph

`T001 → {T002, T003} → T004 → T005 → T006 → T007 → T008 → T009`
