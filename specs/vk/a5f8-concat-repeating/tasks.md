# Tasks: Restore Linked Workspace Breadcrumbs

**Plan**: `./plan.md`

## Phase 1: Resolution primitives

- [x] T001 Add typed `getProject` detail resolution in
  `packages/web-core/src/shared/lib/remoteApi.ts`.
- [x] T002 [P] Generalize explicit project breadcrumb states and unavailable
  label in
  `packages/web-core/src/shared/components/ui-new/containers/navbarBreadcrumbs.ts`.

## Phase 2: Focused contracts

- [x] T003 Add success and confirmed-miss `getProject` tests in
  `packages/web-core/src/shared/lib/remoteApi.test.ts` (depends on T001).
- [x] T004 [P] Add project loading, resolved, and unavailable builder tests in
  `packages/web-core/src/shared/components/ui-new/containers/navbarBreadcrumbs.test.ts`
  (depends on T002).

## Phase 3: Container integration

- [x] T005 Wire collection-first/project-detail-fallback state resolution into
  `packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx`
  (depends on T001, T002).

## Phase 4: Verification

- [x] T006 Run formatter and focused Vitest tests for
  `packages/web-core/src/shared/lib/remoteApi.test.ts` and
  `packages/web-core/src/shared/components/ui-new/containers/navbarBreadcrumbs.test.ts`
  (depends on T003, T004, T005).
- [x] T007 Run relevant frontend type and lint checks (depends on T006).
- [x] T008 Run independent Codex diff review, address confirmed findings, and
  repeat affected verification until no significant findings remain (depends on
  T007).

## Phase 5: Knowledge capture

- [x] T009 Update `wiki/workspace-navbar-breadcrumbs.md` and, only if its summary
  changes, `wiki/INDEX.md`; commit the knowledge-base update (depends on T008).
