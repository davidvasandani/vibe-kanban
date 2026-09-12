# Feature Specification: Repository-scoped Git panel PR links

**Feature dir**: `specs/vk/63e0-git-panel-shows/`
**Status**: Clarified

## Summary

Make each repository row in a multi-repository workspace show only the pull
request that belongs to that repository. This prevents a PR opened for one repo
from falsely making every repo appear shipped and preserves the create-PR action
for repositories that still need one.

## User Stories

- As a developer working across multiple repositories, I want each Git panel
  row to show its own PR state so I can see which portions of the change remain.
- As a developer, I want a repository without a PR to keep its create-PR action
  so I can complete the remaining delivery work.
- As a developer, I want every displayed PR link to open the correct repository
  and PR so I am not sent to an unrelated project.

## Functional Requirements

- FR-1: A repository row with an associated PR displays that PR's number, URL,
  and status.
- FR-2: A repository row without an associated PR does not display a PR from a
  different repository in the workspace.
- FR-3: A repository without a PR retains the actions otherwise available for
  its own commit and branch state.
- FR-4: When repository-scoped PR state has not loaded, the UI represents PR
  state as unknown/absent rather than assigning a workspace-level PR to a row.
- FR-5: If one repository has multiple known PR records, an open PR is displayed
  ahead of a merged PR, consistent with current behavior.
- FR-6: The behavior is consistent for single- and multi-repository workspaces.

## Out of Scope

- Discovering or linking externally/manual-created PRs that Vibe Kanban has not
  associated with a local repository.
- Fixing remote detection errors during PR creation.
- Changing PR persistence, GitHub APIs, or services outside Vibe Kanban.
- Changing the display or action policy for repositories with no commits.

## Acceptance Criteria

- [ ] In a workspace with four repositories and a PR associated with only one,
      exactly that repository row displays the PR link.
- [ ] The other three rows do not display the PR number or URL and retain their
      applicable per-repository actions.
- [ ] Before repository-scoped status loads, no row borrows the workspace
      summary PR.
- [ ] A repository with open and merged records displays the open record.
- [ ] Focused automated tests and frontend static verification pass.

## Open Questions

None.

## Clarifications

- Preserve the current no-changes presentation. The requested correction is PR
  association; a new explicit no-changes state is optional in the reported
  expectation and would expand the UI scope.
- Do not add remote PR discovery in this task. The report identifies it as a
  potentially separate association gap, and issue/workspace PR data lacks the
  trustworthy local repository identity required by this feature's invariant.
