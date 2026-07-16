# Tasks — Workspace breadcrumb issue ID

**Task**: `003-workspace-breadcrumb-issue-id` · **Plan**: [`plan.md`](plan.md)

`[P]` = safely parallelizable after its dependencies are complete because it
does not edit the same files as sibling tasks and does not depend on their
uncommitted changes.

## Implementation status

- [x] T001–T003 Orientation and contract inspection
- [x] T004–T005 Pure breadcrumb contract
- [x] T006–T007 Container wiring
- [x] T008–T011 Focused tests
- [x] T012 Focused Vitest suite (5 tests passed)
- [x] T013 Web-core TypeScript check
- [x] T014 Repository formatter
- [x] T015 Diff inspection (no shared UI or generated-type changes)
- [x] T016 Independent review (no blocking/significant findings)
- [x] T017 Final verification summary captured in implementation handoff

## Stage 0 — Knowledge and orientation

| ID | Task | File(s) | Depends on |
| --- | --- | --- | --- |
| T001 ✓ | Read the feature contract and confirm the required breadcrumb states: resolved `simple_id`, loading defer, unavailable non-link, unchanged unlinked/project-page behavior | `specs/003-workspace-breadcrumb-issue-id/spec.md`, `specs/003-workspace-breadcrumb-issue-id/plan.md`, `specs/003-workspace-breadcrumb-issue-id/research.md` | — |
| T002 ✓ | Inspect the existing breadcrumb construction, navigation calls, and issue-loading flags before editing | `packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx` | T001 |
| T003 ✓ [P] | Inspect the presentational breadcrumb item contract to verify no `packages/ui` API change is needed | `packages/ui/src/components/Navbar.tsx` | T001 |

## Stage 1 — Pure breadcrumb contract

| ID | Task | File(s) | Depends on |
| --- | --- | --- | --- |
| T004 ✓ | Create `navbarBreadcrumbs.ts` with a small pure helper for workspace breadcrumb construction; model issue state as loading, resolved `simple_id`, or unavailable; return `undefined` while linked issue data is loading | `packages/web-core/src/shared/components/ui-new/containers/navbarBreadcrumbs.ts` | T002, T003 |
| T005 ✓ | Ensure the helper emits `Project / simple_id / Workspace` with project and issue callbacks when the linked issue resolves, emits `Project / Issue unavailable / Workspace` with no issue callback after definitive no-match, and preserves existing `Project / Workspace` behavior for workspaces without linked issues | `packages/web-core/src/shared/components/ui-new/containers/navbarBreadcrumbs.ts` | T004 |

## Stage 2 — Container wiring

| ID | Task | File(s) | Depends on |
| --- | --- | --- | --- |
| T006 ✓ | Replace inline breadcrumb item construction with the helper while preserving existing project lookup, workspace label fallback, `RemoteIssueLink` fallback behavior including loading suppression, and `AppNavigation.goToProject` / `goToProjectIssue` routing | `packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx`, `packages/web-core/src/shared/components/ui-new/containers/navbarBreadcrumbs.ts` | T005 |
| T007 ✓ | Compute the matching linked issue once from `projectIssues`; pass an explicit issue-label state to the helper so loading never falls through to a completed `Project / Workspace` trail and UUIDs are never displayed | `packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx` | T006 |

## Stage 3 — Focused tests

| ID | Task | File(s) | Depends on |
| --- | --- | --- | --- |
| T008 ✓ | Add Vitest coverage for resolved linked issue breadcrumbs: exact order `Project / simple_id / Workspace`, exactly one issue item even with long surrounding project/workspace labels, and issue click uses linked project and issue IDs | `packages/web-core/src/shared/components/ui-new/containers/navbarBreadcrumbs.test.ts` | T005 |
| T009 ✓ | Add Vitest coverage for linked issue loading: helper returns no linked breadcrumb trail, no `Project / Workspace` partial hierarchy, and no UUID/fallback issue label | `packages/web-core/src/shared/components/ui-new/containers/navbarBreadcrumbs.test.ts` | T005 |
| T010 ✓ | Add Vitest coverage for completed no-match issue data: exact `Issue unavailable` label between project and workspace, non-clickable issue item, no UUID, and not treated as unlinked | `packages/web-core/src/shared/components/ui-new/containers/navbarBreadcrumbs.test.ts` | T005 |
| T011 ✓ | Add Vitest coverage for unchanged behavior when the workspace has no linked issue and when breadcrumb resolution is inapplicable on project pages | `packages/web-core/src/shared/components/ui-new/containers/navbarBreadcrumbs.test.ts` | T005 |

## Stage 4 — Verification

| ID | Task | File(s) | Depends on |
| --- | --- | --- | --- |
| T012 ✓ [P] | Run the focused Vitest suite for the breadcrumb helper and confirm T008-T011 pass | `packages/web-core/src/shared/components/ui-new/containers/navbarBreadcrumbs.test.ts` | T006, T007, T008, T009, T010, T011 |
| T013 ✓ [P] | Run the web-core/frontend type check to verify helper exports, imports, and `NavbarBreadcrumbItem` usage are type-correct | `packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx`, `packages/web-core/src/shared/components/ui-new/containers/navbarBreadcrumbs.ts` | T006, T007, T008, T009, T010, T011 |
| T014 ✓ [P] | Run `pnpm run format` per repository instructions | — | T006, T007, T008, T009, T010, T011 |
| T015 ✓ | Inspect `git diff` for unrelated file changes and confirm `packages/ui` and generated shared types were not modified | `packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx`, `packages/web-core/src/shared/components/ui-new/containers/navbarBreadcrumbs.ts`, `packages/web-core/src/shared/components/ui-new/containers/navbarBreadcrumbs.test.ts` | T012, T013, T014 |

## Stage 5 — Review and knowledge capture

| ID | Task | File(s) | Depends on |
| --- | --- | --- | --- |
| T016 | Run an independent code review focused on false unavailable states during startup, accidental UUID/fallback labels, incorrect navigation IDs, shared UI regressions, and missing acceptance coverage | `packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx`, `packages/web-core/src/shared/components/ui-new/containers/navbarBreadcrumbs.ts`, `packages/web-core/src/shared/components/ui-new/containers/navbarBreadcrumbs.test.ts` | T015 |
| T017 | Record final verification results and any residual risks in the implementation summary or PR notes; no persistent docs update is required unless review finds behavior that differs from `research.md` | `specs/003-workspace-breadcrumb-issue-id/spec.md`, `specs/003-workspace-breadcrumb-issue-id/plan.md`, `specs/003-workspace-breadcrumb-issue-id/research.md` | T016 |

## Definition of done

All acceptance criteria in [`spec.md`](spec.md) are covered by focused tests;
the linked issue breadcrumb uses only `simple_id` when resolved; loading does
not render a partial `Project / Workspace` trail; unavailable renders exactly
`Issue unavailable` with no issue-opening action; unlinked workspace and
project-page behavior remain unchanged; T012-T016 complete without significant
findings.
