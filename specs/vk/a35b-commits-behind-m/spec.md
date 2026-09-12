# Feature Specification: Commits Behind in the Git Header

**Feature dir**: `specs/vk/a35b-commits-behind-m/`
**Status**: Clarified

## Summary

Show branch staleness in the workspace sidebar's Git section header so users
can see, even while the section is collapsed, when the current workspace branch
is behind its repository's configured target branch. Preserve repository
identity when a workspace contains more than one repository.

## User Stories

- As a developer, I want the collapsed Git header to tell me that my workspace
  branch is behind its target so that I can notice when a rebase may be needed.
- As a developer working across multiple repositories, I want each behind count
  associated with its repository so that I know which branches are stale.
- As a developer whose branch is current, I want the header to remain uncluttered
  so that absence of a warning is easy to understand.

## Functional Requirements

- FR-1: The Git section header MUST show a repository's positive commits-behind
  count for the selected workspace.
- FR-2: The count MUST describe divergence from that repository's configured
  target branch, including remote-prefixed targets, rather than assuming the
  target is literally a local branch named `main`.
- FR-3: In a single-repository workspace, the indicator MUST show the count and
  its behind meaning without redundantly showing the repository name.
- FR-4: In a multi-repository workspace, every positive behind count MUST be
  paired with the corresponding repository display name.
- FR-5: Repositories with zero, missing, or not-yet-loaded behind counts MUST
  NOT produce a header indicator.
- FR-6: When no repository has a positive behind count, the Git header MUST
  retain its current appearance.
- FR-7: The indicator MUST remain visible when the Git section body is collapsed.
- FR-8: The indicator MUST update when the selected workspace or its branch
  status changes.
- FR-9: The indicator MUST remain compact in narrow drawers while preserving a
  complete accessible description of all displayed repository/count pairs.
- FR-10: Repository metadata and status MUST be associated by stable repository
  identity, not by their array positions.

## Out of Scope

- Changing branch divergence calculations or Git fetch behavior.
- Showing ahead, remote-ahead, pull-request, or push state in the Git header.
- Adding rebase or merge actions to the Git header.
- Changing the Git panel's existing repository cards or drawer sizing.
- Modifying homelab deployment or another service.

## Acceptance Criteria

- [ ] Given one repository that is 3 commits behind, the Git header shows
      `3 behind` while expanded and collapsed.
- [ ] Given one repository that is 0 commits behind, the Git header shows no
      behind indicator.
- [ ] Given repositories `web` 2 commits behind and `server` 5 commits behind,
      the Git header identifies both as `web 2` and `server 5`.
- [ ] Given multiple repositories where only one is behind, the visible value
      still names that repository.
- [ ] Given repository and status arrays in different orders, each count is
      shown beside the correct repository name.
- [ ] The full meaning remains available to assistive technology/title text
      when visible content is truncated.
- [ ] Focused UI tests and required repository verification pass.

## Open Questions

None. `/speckit.clarify` resolved the initial presentation questions from the
request and existing drawer conventions; see `clarifications.md`.
