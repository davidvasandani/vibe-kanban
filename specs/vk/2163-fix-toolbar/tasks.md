# Tasks: Expand Mobile Workspace Toolbar

**Plan**: `./plan.md`

Tasks are dependency ordered. `[P]` marks work that is safe to perform together
within the same dependency layer because it touches independent files.

## Phase 1: Regression Contract

- [x] T001 Extend the real mobile Navbar rendered-DOM coverage with assertions
  for a flexible/scrollable workspace-toolbar region, distributed tool tabs,
  non-shrinking trailing controls, and unchanged active accessibility state in
  `packages/web-core/src/shared/components/ui-new/Navbar.mobile.test.tsx`.
- [x] T002 Run the focused Navbar test before implementation and confirm the new
  layout assertion fails for the current intrinsic-width toolbar (no file
  changes).

## Phase 2: Implementation

- [x] T003 Update the mobile workspace branch in
  `packages/ui/src/components/Navbar.tsx` to grow the toolbar region, distribute
  surplus width across visible tool tabs, preserve usable minimum widths and
  horizontal overflow, and keep the trailing region fixed (depends on T001,
  T002).
- [x] T004 Run the focused Navbar test and confirm all mobile navbar behavior
  passes after T003 (no file changes).

## Phase 3: Validation

- [x] T005 [P] Run `@vibe/ui` TypeScript and ESLint checks (depends on T004; no
  file changes).
- [x] T006 [P] Run `@vibe/web-core` TypeScript check and focused/full relevant
  tests (depends on T004; no file changes).
- [x] T007 Run repository formatting and diff validation, then inspect the final
  scoped diff (depends on T005, T006; formatting may touch only authored files).

## Phase 4: Review and Delivery

- [x] T008 Run an independent Codex review of the task diff, fix every confirmed
  significant finding in the relevant authored files, and repeat validation and
  review until clean (depends on T007).
- [x] T009 Distill reusable responsive-toolbar knowledge into
  `docs/knowledge-base/` and refresh `docs/knowledge-base/INDEX.md`, tagging the
  page with `vk/2163-fix-toolbar`; commit the knowledge base (depends on T008).
- [x] T010 Commit remaining task changes, push the branch, open a pull request
  against the base branch, satisfy merge requirements, and merge it (depends on
  T009).

## Dependency Graph

`T001 → T002 → T003 → T004 → {T005, T006} → T007 → T008 → T009 → T010`
