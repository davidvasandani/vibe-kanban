# Implementation Plan: Restore Linked Workspace Breadcrumbs

**Spec**: `./spec.md`
**Status**: Ready

## Technical Context

The change is in the React/TypeScript shared frontend. `NavbarContainer.tsx`
resolves remote entities using Electric-backed collections and TanStack Query;
`packages/ui/src/components/Navbar.tsx` renders prepared breadcrumb items for
both local and remote web consumers. The remote Rust server already exposes
authenticated `GET /v1/projects/{project_id}` and `GET /v1/issues/{issue_id}`
routes, so no backend or generated-type change is needed.

## Architecture & Approach

1. Add `getProject(projectId)` beside `getIssue(issueId)` in
   `packages/web-core/src/shared/lib/remoteApi.ts`. It uses the existing project
   detail route and the established null-on-404/throw-on-other-error convention.
2. Cover the helper in `remoteApi.test.ts` using the same authenticated-fetch
   test harness as `getIssue`.
3. In `NavbarContainer.tsx`, keep the all-organization project collection as the
   primary source. Once it is ready and misses the linked project, start a
   TanStack Query keyed by the project UUID for authoritative detail resolution.
4. Derive a project state: collection/detail loading defers the trail; a found
   project supplies its name and project navigation callback; settled absence or
   request failure supplies an unavailable project state.
5. Generalize `navbarBreadcrumbs.ts` from a nullable project object to explicit
   project `loading`/`resolved`/`unavailable` state, analogous to its issue
   state. This preserves hierarchy without manufacturing an identity.
6. Extend `navbarBreadcrumbs.test.ts` for loading and unavailable project cases
   while preserving existing issue and unlinked coverage.

## Data Model

See `./data-model.md`. No persisted data changes.

## Contracts

See `./contracts/project-detail.md`. The server contract already exists; this
feature adds only a typed frontend consumer.

## Research Notes

See `./research.md`. No dependency is added.

## Constitution Check

- II: focused pure-builder and API-helper tests check behavior.
- III/VI: the plan reuses the existing project detail endpoint, query library,
  issue fallback pattern, and presentational navbar.
- IV: data resolution remains in web-core; packages/ui receives prepared items.
- VII: async collection absence is not treated as relationship absence, and
  UUIDs remain lookup/navigation values only.
- XIV: validation uses the lockfile-defined frontend toolchain.
- XXI: project detail follows the existing `getIssue` resolution/error rule.

No constitution deviation or open question remains.

## Risks & Dependencies

- Query retry behavior can keep `isLoading` false while a retry is scheduled;
  explicit state derivation must settle to unavailable without hiding forever.
- Project and issue fallbacks can run concurrently; their state must combine
  deterministically and avoid rendering a partial trail.
- `Project unavailable / Workspace` requires at least two breadcrumb items so it
  remains compatible with the builder's existing non-singleton rule.
