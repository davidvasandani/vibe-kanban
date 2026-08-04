# Feature Specification: Restore Linked Workspace Breadcrumbs

**Feature dir**: `specs/vk/a5f8-concat-repeating/`
**Status**: Clarified

## Summary

Restore the full breadcrumb trail on linked workspace pages so users retain
project and issue context even when asynchronously loaded collections observe
the workspace relationship before they observe the linked project or issue.

## User Stories

- As a user working in a project-linked workspace, I want to see the project,
  issue when applicable, and workspace hierarchy so I know where the session
  belongs.
- As a user navigating between a workspace and its source project or issue, I
  want resolved breadcrumb entries to be actionable so I can return directly to
  the relevant board context.
- As a user during synchronization or after a stale link, I want the header to
  represent loading or unavailability truthfully rather than silently dropping
  the linked hierarchy or displaying an internal identifier.

## Functional Requirements

- FR-1: A workspace with a linked project must display a breadcrumb containing
  the human-readable project name followed by the workspace label once the
  project identity has been resolved.
- FR-2: A workspace with both a linked project and linked issue must display the
  human-readable project name, issue `simple_id`, and workspace label in that
  order once both identities have been resolved.
- FR-3: A linked entity's temporary absence from an asynchronously loaded
  collection must not be treated as proof that the relationship is absent or
  permanently unavailable.
- FR-4: The system must distinguish initial loading, resolved identity, and
  confirmed unavailability for linked breadcrumb entities.
- FR-5: The full linked trail must be deferred while required human-readable
  identity is still being resolved; a misleading partial hierarchy must not be
  shown.
- FR-6: A resolved project breadcrumb must navigate to the linked project, and a
  resolved issue breadcrumb must navigate to the linked issue in that project.
- FR-7: Internal project and issue UUIDs must be used only for lookup and
  navigation and must never be rendered as breadcrumb labels.
- FR-8: A confirmed unavailable issue must retain its hierarchy position with
  the existing non-actionable `Issue unavailable` label.
- FR-8a: A confirmed unavailable project must retain its hierarchy position
  with a non-actionable `Project unavailable` label followed by the workspace
  label. If the workspace also names an issue, its resolved or unavailable
  position remains between those two entries.
- FR-9: Unlinked workspaces and project-page navbar behavior must remain
  unchanged.
- FR-10: Desktop and mobile navbar layouts must consume the same prepared
  breadcrumb state.
- FR-11: Automated tests must cover collection misses followed by authoritative
  resolution, confirmed absence, resolved label order, click behavior, and UUID
  non-disclosure.

## Out of Scope

- Redesigning navbar layout or breadcrumb styling.
- Changing how workspaces are linked to projects or issues.
- Changing synchronization infrastructure or entity identifier generation.
- Changing services outside Vibe Kanban or its deployment configuration.
- Introducing a user-facing UUID fallback.

## Acceptance Criteria

- [ ] A project-linked workspace no longer falls back to only its workspace
  title when the project collection temporarily lacks the linked project.
- [ ] A resolved issue-linked workspace produces `Project / ISSUE-ID /
  Workspace` in focused tests.
- [ ] Resolved project and issue entries invoke navigation with their linked
  UUIDs.
- [ ] Loading does not produce a partial linked hierarchy.
- [ ] Confirmed unavailable issue behavior remains explicit and non-actionable.
- [ ] Confirmed unavailable project behavior is explicit and non-actionable
  rather than collapsing to only the workspace title.
- [ ] No tested state renders a project or issue UUID as a label.
- [ ] Unlinked workspace and project-page behavior remains unchanged.
- [ ] Relevant formatting, tests, type checks, and lint checks pass.

## Clarifications

- C1: A confirmed stale/deleted project is represented by a non-actionable
  `Project unavailable` breadcrumb. This preserves the known relationship
  hierarchy and distinguishes confirmed absence from an unlinked workspace.
- C2: An authoritative detail-request error is a settled unavailable state for
  this bounded navbar resolution attempt; normal query retry policy may recover
  it later, but the navbar must not remain indefinitely hidden.

## Open Questions

None.
