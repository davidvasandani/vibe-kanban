# Technical Spec: Restore Workspace Breadcrumbs

Task id: `f195-bread-crumbs-are`

## Summary

Restore the project and linked-issue breadcrumbs in the Vibe Kanban workspace
navbar when a local workspace is associated with a remote project/issue. The
workspace header must keep showing useful navigation while remote project data
is temporarily absent or still synchronizing, without exposing UUIDs as labels.

## Problem

Some workspace pages render only the workspace title (for example,
`pear.vasandani.dev` followed by the branch name) even though the workspace is
linked to a project or issue. The navbar currently suppresses the complete
breadcrumb whenever the linked project cannot be found in the organization-wide
project query. That makes the project and issue navigation disappear and causes
the header to fall back to the plain workspace title.

## Scope

- Diagnose breadcrumb resolution in the shared web navbar.
- Keep `Project / Issue / Workspace` visible for linked workspaces whenever a
  trustworthy human-readable project label is available.
- Handle loading, synchronization races, stale links, and unavailable records
  explicitly.
- Preserve project and issue navigation for resolved breadcrumb entries.
- Add focused automated coverage for the failure and fallback states.

## Out of Scope

- Changes to any service other than Vibe Kanban.
- Deployment or Nix changes unless investigation proves the Vibe Kanban module
  itself is responsible.
- Changes to project/issue linking semantics, organization selection, routing,
  or remote synchronization.
- Displaying raw database UUIDs as user-facing breadcrumb labels.
- Broad navbar redesign.

## Functional Requirements

1. A workspace linked to a known project displays the project name and workspace
   label in the navbar.
2. A workspace linked to a known issue displays project name, issue simple ID,
   and workspace label in that order.
3. Resolved project and issue crumbs remain clickable and navigate to their
   corresponding project destinations.
4. Temporary query/synchronization ordering must not permanently collapse the
   breadcrumb to the workspace title.
5. Missing or stale remote entities use an explicit, stable fallback rather than
   leaking an internal UUID or silently presenting an incorrect identity.
6. Unlinked workspaces retain the existing plain-title behavior.
7. Desktop and mobile navbar layouts consume the same resolved breadcrumb data.

## Technical Direction

Keep remote-data resolution in `NavbarContainer` and presentation in the shared
`Navbar` component. Extract or extend pure breadcrumb-building logic so resolved,
loading, and unavailable states can be tested without rendering the full app.
Prefer existing project and issue data sources and targeted API fallback queries;
do not introduce a new backend contract unless the existing APIs cannot provide
the linked entity identity.

The implementation must distinguish an initial loading state from a completed
query that did not contain the linked entity. A completed collection query is
not proof that a linked record is permanently unavailable because collection
sync and workspace-link sync may arrive in different orders.

## Acceptance Criteria

- The reported linked-workspace case renders breadcrumbs instead of only the
  workspace title.
- Breadcrumb ordering and click behavior are covered by focused tests.
- Loading and unavailable states are covered by focused tests.
- Unlinked workspace behavior is unchanged.
- Frontend formatting, focused tests, and relevant type/lint checks pass.
- An independent Codex review reports no significant findings.

## Risks

- Treating an empty synchronized collection as authoritative can reproduce the
  race after initial load.
- A fallback fetch may retry excessively or flash conflicting labels if its
  enablement and cache key are unstable.
- Rendering a placeholder too early can replace the desired loading behavior;
  rendering nothing indefinitely recreates the missing-breadcrumb bug.

## Verification

- Unit-test the breadcrumb state builder and any fallback-resolution helper.
- Run the focused frontend test files.
- Run the repository-prescribed frontend checks and formatter after installing
  lockfile-defined dependencies if needed.
- Inspect the final diff for unrelated service or deployment changes.
