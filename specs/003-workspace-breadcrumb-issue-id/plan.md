# Implementation Plan: Workspace Breadcrumb Issue ID

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context

- Frontend stack: React + TypeScript, TanStack Router app shells, Vite/Vitest.
- Relevant UI boundary:
  - `packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx`
    owns application data lookup and breadcrumb construction.
  - `packages/ui/src/components/Navbar.tsx` is presentational and renders a
    prepared `NavbarBreadcrumbItem[]`.
- Relevant data sources:
  - `useWorkspaceContext()` provides the selected local workspace and create
    mode state.
  - `useUserContext().workspaces` contains remote workspace records linked to
    local workspaces, including `project_id` and optional `issue_id`.
  - `useAllOrganizationProjects()` resolves linked project labels across
    organizations.
  - `useShape(PROJECT_ISSUES_SHAPE, { project_id })` resolves project issues and
    exposes `simple_id`.
- Relevant navigation contract:
  - `useAppNavigation().goToProject(projectId)` opens the project.
  - `useAppNavigation().goToProjectIssue(projectId, issueId)` opens the linked
    issue in that project.
- Constraint from prior knowledge: asynchronous Electric/REST-backed collection
  data may be temporarily absent; absence while loading must not collapse an
  issue-linked breadcrumb to `Project / Workspace`.
- No new top-level dependencies are required.

## Architecture & Approach

Implement the fix in the shared navbar container layer, not the presentational
navbar component.

1. Preserve the existing workspace/project eligibility checks in
   `NavbarContainer`:
   - only workspace pages, not create mode,
   - linked remote workspace has a `project_id`,
   - linked issue behavior applies only when `issue_id` is present.

2. Make breadcrumb construction distinguish three issue states:
   - `loading`: linked issue exists, but the issue shape is still loading.
   - `resolved`: matching project issue exists and has `simple_id`.
   - `unavailable`: loading has completed and no matching issue record with
     displayable `simple_id` is available.

3. While issue state is `loading`, return `undefined` breadcrumbs so the linked
   trail is deferred instead of rendering `Project / Workspace`.

4. When issue state is `resolved`, render:
   - project crumb: project name, clickable via `goToProject(linkedProjectId)`;
   - issue crumb: `issue.simple_id`, clickable via
     `goToProjectIssue(linkedProjectId, linkedIssueId)`;
   - workspace crumb: existing workspace name/branch label.

5. When issue state is `unavailable`, render:
   - project crumb as above;
   - issue crumb labeled exactly `Issue unavailable`;
   - workspace crumb as above.

   The unavailable issue crumb should not expose `linkedIssueId` as label. It
   should be non-navigable unless product direction explicitly changes; the spec
   only requires navigation for visible issue breadcrumbs representing a linked
   issue identity and separately defines `Issue unavailable` as the definitive
   failure label.

6. Preserve current behavior for workspaces without `issue_id`:
   - do not synthesize an issue crumb;
   - keep existing project/workspace breadcrumb behavior.

7. Preserve current behavior for project pages and project subroutes:
   - `isOnProjectPage` continues to bypass workspace breadcrumb resolution.

8. Keep `RemoteIssueLink` fallback behavior compatible:
   - it may continue rendering only when no breadcrumbs are present and the
     remote workspace has an issue id.
   - preserve the existing loading guard so the fallback slot does not render a
     temporary issue affordance while linked issue breadcrumbs are deferred.

Recommended implementation shape:

- Add a small pure helper near `NavbarContainer`, or in a nearby frontend-only
  module if direct component tests become too brittle. Example responsibility:
  resolve a `NavbarBreadcrumbItem[] | undefined` from project, issue-loading
  state, issue list, workspace label, and click callbacks.
- Keep the helper local to navbar behavior; do not alter Electric collection
  hooks, generated types, route definitions, or `packages/ui`.

## Data Model

No new data model document is needed. The feature uses existing remote workspace
fields (`project_id`, `issue_id`), project fields (`id`, `name`), and issue
fields (`id`, `simple_id`).

## Contracts

No new API or UI package contract is needed. The existing
`NavbarBreadcrumbItem` contract is sufficient:

- `label: string`
- optional `onClick`

The implementation should preserve that contract and avoid adding loading,
disabled, or issue-specific fields to `packages/ui`.

## Research Notes

See `./research.md`.

## Constitution Check

- Clarity over cleverness: keep the fix in the existing navbar data container
  and make loading/resolved/unavailable states explicit.
- Test the contract: add focused frontend tests for resolved issue, loading
  defer, unavailable issue, unlinked workspace, and navigation callback behavior.
- Small, reversible steps: no data model, route, generated type, dependency, or
  presentational redesign changes.
- Shared-component boundaries are law: preserve the `packages/ui`
  presentational boundary and resolve issue labels in `web-core`.
- Don't rebuild what shipped: reuse existing navigation APIs, Electric issue
  shape data, and current fallback guards.
- Workspace breadcrumbs preserve issue identity: issue-linked workspace
  breadcrumbs display only the resolved issue `simple_id` or the explicit
  unavailable state; UUIDs and other fallback labels are not used.
- Dependencies: no new dependency planned.

No constitution deviations are identified.

## Risks & Dependencies

- The main risk is confusing temporary absence with definitive absence. Tests
  should directly control `isProjectIssuesLoading` and issue list contents so
  the distinction is explicit.
- Rendering `Issue unavailable` too early would regress loading behavior; the
  unavailable state must require issue loading to be complete.
- Current `RemoteIssueLink` fallback must stay suppressed while linked issue
  breadcrumbs are deferred; otherwise loading could expose an inconsistent
  temporary issue affordance outside the breadcrumb trail.
- Full DOM component tests for `NavbarContainer` may require extensive provider
  mocking. If that becomes brittle, prefer extracting a pure breadcrumb resolver
  and testing it directly, with one integration-style render test only if it is
  low-maintenance.
