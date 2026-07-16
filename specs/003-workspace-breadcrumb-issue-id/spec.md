# Feature Specification: Workspace Breadcrumb Issue ID

**Feature Branch**: `003-workspace-breadcrumb-issue-id`
**Created**: 2026-07-15
**Status**: Clarified
**Input**: Feature brief from `/var/tmp/vibe-kanban/worktrees/6c5c-bread-crumbs-sho/SPEC.md`

## User Scenarios & Testing

### Primary User Story

As a user viewing a workspace that was created from or linked to an issue, I
need the workspace breadcrumb trail to show the issue's human-readable ID so I
can keep the workspace tied to the task I am working on without seeing a UUID,
misleading fallback label, or partial `Project / Workspace` trail.

### Acceptance Scenarios

1. **Given** a workspace is linked to a project and a linked issue, **When** the
   workspace page renders and the full issue record is available, **Then** the
   breadcrumbs show `Project / Issue ID / Workspace`, using the issue's
   `simple_id` value for the issue breadcrumb.

2. **Given** a workspace is linked to a project and a linked issue, **When** the
   workspace page renders while the matching issue record is still loading,
   **Then** the linked breadcrumb trail is deferred and the app does not render
   `Project / Workspace`, the issue UUID, or any fallback issue label.

3. **Given** a workspace is linked to a project and a linked issue, **When** the
   issue data source has finished loading and the matching issue record truly
   cannot be resolved, **Then** the breadcrumbs show
   `Project / Issue unavailable / Workspace`, with the issue item rendered as an
   explicit unavailable state rather than a link or identifier.

4. **Given** a visible resolved issue breadcrumb in a workspace breadcrumb
   trail, **When** the user selects it, **Then** the app opens the linked issue
   in its project.

5. **Given** a workspace has no linked issue, **When** the workspace page
   renders, **Then** the breadcrumb behavior remains unchanged and no synthetic
   issue breadcrumb is added.

### Edge Cases

- Issue data is temporarily absent because asynchronous collection loading has
  not completed; no linked breadcrumb trail is rendered until the issue state is
  resolved or declared unavailable.
- Issue data is unavailable because a completed collection lookup does not
  return the linked issue; the breadcrumb shows `Issue unavailable`.
- Issue data becomes available after the initial render and contains a
  `simple_id`; the displayed issue breadcrumb updates to that `simple_id`.
- The linked issue relationship is present but only raw relationship UUIDs are
  available; those UUIDs are navigation inputs only and are not displayed as
  issue labels.
- The workspace is linked to a project but not to an issue.
- Existing project-page breadcrumbs are rendered outside the workspace issue
  context.
- Breadcrumb item text is shortened by responsive layout; the issue breadcrumb
  must still represent the issue identity.

## Requirements

### Functional Requirements

- **FR-001**: On a workspace page with both a linked project and linked issue,
  the system MUST render an issue breadcrumb between the project breadcrumb and
  the workspace breadcrumb only after the issue label state is resolved to a
  human-readable `simple_id` or an explicit unavailable state.
- **FR-002**: When the linked issue record is available and includes a
  human-readable `simple_id`, the system MUST use that `simple_id` as the issue
  breadcrumb label.
- **FR-003**: While the linked issue record is loading, the system MUST defer
  the linked breadcrumb trail rather than render a partial `Project / Workspace`
  hierarchy.
- **FR-004**: The system MUST NOT use UUIDs, database identifiers, branch names,
  workspace names, project names, or any other non-issue-display value as an
  issue breadcrumb fallback.
- **FR-005**: Selecting the issue breadcrumb MUST navigate to or open the linked
  issue within its linked project when the issue breadcrumb represents a
  resolved issue.
- **FR-006**: The system MUST preserve existing breadcrumb behavior for
  workspaces without linked issues.
- **FR-007**: The system MUST preserve existing breadcrumb behavior for project
  pages and other non-workspace surfaces.
- **FR-008**: The breadcrumb rendering component MUST continue to receive and
  display a prepared breadcrumb item list; issue label resolution belongs in the
  application container layer.
- **FR-009**: The feature MUST be covered by focused automated tests for
  resolved issue data, loading issue data, and unavailable issue data cases.
- **FR-010**: Once the issue data source has completed loading and confirms that
  the linked issue cannot be resolved, the system MUST render an explicit
  unavailable issue breadcrumb between the project and workspace breadcrumbs.
- **FR-011**: The unavailable issue breadcrumb MUST be visually and semantically
  distinct from a resolved issue breadcrumb: its label is `Issue unavailable`,
  it has no issue-opening action, and it MUST NOT imply that the workspace is
  unlinked.
- **FR-012**: Temporary absence from an asynchronous project issue collection
  MUST NOT be treated as proof that the workspace has no linked issue or that
  the issue is unavailable; unavailable state is only allowed after the relevant
  issue data source has finished loading.
- **FR-013**: Responsive or space-constrained breadcrumb rendering MUST preserve
  a distinct issue breadcrumb item whose label represents the issue identity;
  surrounding project or workspace label shortening MUST NOT replace, collapse,
  or hide the issue identity.

### Non-Goals

- Changing how issue identifiers are generated, stored, or synchronized.
- Redesigning the navbar, breadcrumb visuals, spacing, or responsive layout.
- Changing project breadcrumb labels.
- Changing workspace breadcrumb labels.
- Changing the underlying project issue collection loading or fallback
  mechanism.
- Adding new top-level dependencies.
- Introducing a new human-readable fallback identifier for issues.

## Measurable Acceptance Criteria

- **AC-001**: In automated tests, a workspace with linked project and linked
  issue plus a resolved issue record renders exactly one issue breadcrumb
  between the project and workspace breadcrumbs.
- **AC-002**: In automated tests, the resolved issue breadcrumb label equals the
  linked issue record's `simple_id`.
- **AC-003**: In automated tests, a workspace with linked project and linked
  issue while issue records are loading renders no partial
  `Project / Workspace` breadcrumb trail and renders no UUID or fallback issue
  label.
- **AC-004**: In automated tests, a workspace with linked project and linked
  issue after issue loading completes with no matching issue record renders
  exactly one unavailable issue breadcrumb between the project and workspace
  breadcrumbs.
- **AC-005**: In automated tests, selecting the issue breadcrumb uses the linked
  project and linked issue identifiers for navigation when the issue is
  resolved.
- **AC-006**: In automated tests, a workspace without a linked issue renders no
  issue breadcrumb.
- **AC-007**: In automated tests, existing project-page breadcrumb behavior is
  unchanged.
- **AC-008**: In automated tests, the unavailable issue breadcrumb label is
  exactly `Issue unavailable` and has no issue-opening action.
- **AC-009**: In automated tests, no issue-linked workspace state renders a
  breadcrumb trail equivalent to `Project / Workspace` while the issue is
  loading or unavailable.
- **AC-010**: In automated tests, a resolved issue-linked workspace with long
  surrounding project or workspace labels still produces a distinct issue
  breadcrumb item labeled with the linked issue record's `simple_id`.

## Assumptions

- `simple_id` is the preferred human-readable issue identifier whenever it is
  available.
- The existing breadcrumb item contract can represent resolved and unavailable
  issue labels without changes to the presentational breadcrumb component.
- A linked workspace's project and issue identifiers are sufficient for
  navigation, but issue identifiers are not suitable display labels.

## Dependencies

- Workspace pages already know their linked project relationship.
- Workspace pages already know whether they are linked to an issue, including
  the linked issue identifier needed for navigation.
- Project issue records, when loaded, expose a `simple_id` suitable for display.
- The frontend can distinguish an issue collection that is still loading from
  one that has completed without returning the linked issue.

## Validation

- Add focused frontend tests that exercise breadcrumb construction or rendering
  for resolved issue data, loading issue data, unavailable issue data, unlinked
  workspaces, and project-page behavior.
- Verify the issue breadcrumb remains clickable and routes using the linked
  project and issue identifiers only in the resolved issue case.
- Do not rely on timing-sensitive collection loading behavior to prove loading
  or unavailable behavior; test those states directly.
- Verify no loading or unavailable state displays the linked issue UUID or a
  partial `Project / Workspace` breadcrumb trail.
- Verify long surrounding project or workspace labels do not collapse the
  resolved issue breadcrumb into a project or workspace label.

## Clarifications (resolved)

- "Always" means an issue-linked workspace never presents a completed
  `Project / Workspace` breadcrumb that erases the issue position.
- Loading is not an unavailable result: the linked trail is deferred until the
  human-readable ID is available or issue loading completes without the linked
  issue.
- A definitive resolution failure uses the exact non-navigable label
  `Issue unavailable`; raw UUIDs are never substituted for `simple_id`.
