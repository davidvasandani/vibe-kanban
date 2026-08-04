# Implementation Plan: Restore Workspace Breadcrumbs

Task id: `f195-bread-crumbs-are`

1. Confirm the failing state from the navbar data flow: a linked project ID is
   present, the organization-wide project collection has completed without that
   row, and `buildWorkspaceBreadcrumbs` returns no breadcrumb because `project`
   is absent.
2. Add an authenticated `getProject(projectId)` frontend API helper against the
   existing `GET /v1/projects/{project_id}` endpoint. Match `getIssue` semantics:
   return the typed entity on success, `null` on a confirmed 404, and throw a
   parsed error for other failures.
3. Add focused API-helper tests for successful project resolution and confirmed
   absence, alongside the existing issue-detail tests.
4. In `NavbarContainer`, trigger the authoritative project-detail query only
   after the enabled organization project collection has finished and did not
   contain the linked project. Use a stable query key containing the project ID.
5. Resolve the project from the collection first and the detail response second.
   Treat both collection loading and an in-flight detail fallback as breadcrumb
   loading. After the detail query settles, pass the resolved project or the
   existing unavailable outcome through the breadcrumb builder without leaking
   the UUID.
6. Extend pure breadcrumb tests as needed to lock in the full linked hierarchy,
   deferred loading state, and unavailable behavior. Avoid changing unlinked
   workspace or navbar layout behavior.
7. Install lockfile-defined dependencies if absent, format touched code, and run
   focused Vitest coverage plus relevant frontend type/lint checks.
8. Run SpecKit analysis before implementation, execute tasks in dependency order,
   then run an independent Codex diff review. Address confirmed findings and
   repeat verification until no significant findings remain.
9. Update the existing workspace breadcrumb knowledge page with the analogous
   project-shape race/fallback lesson, add this task ID, refresh the index only if
   its summary needs adjustment, and commit the knowledge-base update separately.

## Expected files

- `packages/web-core/src/shared/lib/remoteApi.ts`
- `packages/web-core/src/shared/lib/remoteApi.test.ts`
- `packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx`
- `packages/web-core/src/shared/components/ui-new/containers/navbarBreadcrumbs.test.ts`
  (only if additional pure-state coverage is required)
- `wiki/workspace-navbar-breadcrumbs.md`
- `wiki/INDEX.md` (only if its topic summary changes)

No homelab deployment file is expected to change.
