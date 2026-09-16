# Feature Specification: Search by Issue ID

**Feature dir**: `specs/vk/b1df-searching-by-iss/`
**Status**: Clarified

## Summary

Allow users to find any issue they can access by entering its visible issue ID
in Global Search. Today the same identifier is prominent throughout the product
but Global Search omits issue records, forcing users to first locate the correct
organization and project.

## User Stories

- As a user who knows an issue ID, I want Global Search to show that issue so I
  can open it without navigating through its organization and project.
- As a user working across organizations, I want issue results to show their
  project context so I can distinguish similarly named work.
- As an organization member, I want search to reveal only issues I am allowed to
  access so cross-organization search does not weaken authorization.

## Functional Requirements

- FR-001: Global Search MUST return accessible issues whose human-readable issue
  ID contains the entered query, using case-insensitive literal matching.
- FR-002: An issue result MUST display the human-readable issue ID, issue title,
  and owning organization/project context.
- FR-003: Issue results MUST be presented in a distinct `Issues` result group.
- FR-004: Selecting an issue result MUST open that exact issue in its owning
  project.
- FR-005: When the issue belongs to a different organization, selection MUST
  coordinate the organization change before navigation so the destination is
  not replaced by normal organization-switch routing.
- FR-006: Search MUST return only issues in organizations accessible to the
  requesting user.
- FR-007: Issue results MUST obey the same per-category result bound and
  truncation indication as existing Global Search categories.
- FR-008: Adding issue results MUST preserve existing organization, project,
  workspace, chat, partial-failure, cancellation, and deadline behavior.
- FR-009: Internal issue UUIDs MUST be used for identity and routing but MUST NOT
  be substituted for the human-readable issue ID in the result label.

## Out of Scope

- Searching issue titles, descriptions, comments, tags, internal UUIDs, or
  external tracker identifiers.
- Changing the project kanban filter or issue-ID generation.
- Searching local host databases for issue records.
- Deployment or hosting changes.

## Acceptance Criteria

- [x] Entering an accessible issue's exact visible ID returns it under `Issues`.
- [x] Entering the same ID with different letter casing returns the same issue.
- [x] The result presents the visible ID, title, and project context without
      showing the issue UUID as its label.
- [x] Selecting the result opens `/projects/{project_id}/issues/{issue_id}` and
      preserves that destination across an organization change.
- [x] An issue outside the requester's organization memberships is not returned.
- [x] More than 20 matching issues returns at most 20 issue rows and marks the
      response truncated.
- [x] Existing Global Search regression tests continue to pass.

## Open Questions

None.
