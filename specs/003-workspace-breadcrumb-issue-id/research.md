# Research: Workspace Breadcrumb Issue ID

## Sources Inspected

- `specs/003-workspace-breadcrumb-issue-id/spec.md`
- `/var/tmp/vibe-kanban/worktrees/6c5c-bread-crumbs-sho/PRIOR_KNOWLEDGE.md`
- `assets/speckit/memory/constitution.md`
- `assets/speckit/templates/plan-template.md`
- `packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx`
- `packages/ui/src/components/Navbar.tsx`
- `packages/web-core/src/shared/lib/routes/appNavigation.ts`
- `packages/web-core/src/shared/hooks/useAppNavigation.ts`
- `packages/local-web/src/app/navigation/AppNavigation.ts`
- `packages/remote-web/src/app/navigation/AppNavigation.ts`
- `packages/web-core/src/shared/hooks/useAllOrganizationProjects.ts`
- `packages/web-core/src/shared/integrations/electric/hooks.ts`
- `packages/remote-web/src/app/layout/RemoteNavbarContainer.tsx`
- `packages/web-core/vitest.config.ts`
- `packages/remote-web/vitest.config.ts`
- Representative frontend tests under `packages/remote-web/src/test/` and
  `packages/web-core/src/shared/lib/`.

## Decision: Keep Logic in `NavbarContainer`

**Decision**: Resolve issue breadcrumb labels in
`packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx`
or a helper owned by that container.

**Rationale**:

- The presentational `Navbar` already accepts a prepared breadcrumb item list
  and only renders labels/click handlers.
- Prior knowledge states application data and navigation wiring belong in
  web-core containers.
- The existing implementation already performs project and issue lookup in
  `NavbarContainer`, so this is the smallest change.

**Alternatives considered**:

- Change `packages/ui/src/components/Navbar.tsx`: rejected because it would
  push application-specific issue resolution into a presentational component.
- Change Electric collection behavior: rejected because the issue is local
  breadcrumb state handling, and prior knowledge warns against broad collection
  behavior changes.

## Decision: Treat Loading and Unavailable as Distinct States

**Decision**: The implementation should explicitly separate:

- issue shape still loading,
- matching issue found with `simple_id`,
- loading complete but no usable matching issue.

**Rationale**:

- The spec requires deferring the linked breadcrumb trail while loading.
- The current code waits only on `isProjectIssuesLoading`; once loading is false
  and the issue is missing, it skips the issue crumb and can render
  `Project / Workspace`.
- A definitive miss must render `Issue unavailable` and must never expose the
  raw UUID.

**Alternatives considered**:

- Use `linkedIssueId` as a fallback label: rejected by FR-004 because raw UUIDs
  must not be displayed as human-readable issue IDs.
- Always render `Issue unavailable` when no issue is found: rejected while
  loading because temporary absence is expected with async collections.

## Decision: Do Not Add Data Model or API Contracts

**Decision**: No `data-model.md` or `contracts/` artifact is needed.

**Rationale**:

- Existing fields already provide all required relationships and labels:
  remote workspace `project_id`/`issue_id`, project `name`, issue `simple_id`.
- The existing `NavbarBreadcrumbItem` interface can represent clickable project
  and issue crumbs plus the unavailable label.
- No backend, generated type, route, or persistence changes are required.

## Decision: Prefer Focused Frontend Tests Around Breadcrumb Resolution

**Decision**: Cover the behavior with focused frontend tests that control inputs
  directly, preferably by extracting a pure breadcrumb resolver if rendering the
  full `NavbarContainer` requires excessive provider mocking.

**Rationale**:

- `packages/web-core` has Vitest coverage for pure logic under a node test
  environment.
- DOM tests using Testing Library currently live under app packages such as
  `packages/remote-web`, whose Vitest config uses jsdom.
- The acceptance criteria are mostly about breadcrumb item construction and
  navigation callback selection, which can be tested without timing-sensitive
  Electric internals.

**Minimum test cases**:

- Resolved issue record renders exactly `Project / SIMPLE-ID / Workspace`.
- Loading linked issue returns no linked breadcrumb trail, preventing
  `Project / Workspace`.
- Definitively unresolved issue renders `Issue unavailable` and does not render
  the raw issue UUID.
- Clicking the resolved issue crumb calls `goToProjectIssue` with linked
  project and linked issue IDs.
- Workspace without `issue_id` does not add an issue crumb.
- Project-page breadcrumb behavior remains unchanged by ensuring the resolver
  or container path is bypassed when `isOnProjectPage` is true.

## Dependency Decision

**Decision**: Add no new dependency.

**Rationale**: Existing React, Vitest, Testing Library, and app navigation APIs
are sufficient.
