# Technical Spec: Repository-scoped PR state in the Git panel

## Problem

`GitPanelContainer` uses a workspace-level summary PR as a fallback for every
repository while per-repository branch status is unavailable. In a multi-repo
workspace, that broadcasts one repository's PR number, URL, and status to all
rows. It also suppresses the primary create/link actions on unrelated rows.

The persisted local PR model is already repository-scoped: each PR-derived
`Merge` in `RepoBranchStatus.merges` belongs to the status entry identified by
`repo_id`. The workspace summary intentionally represents only a single/latest
workspace PR and cannot establish repository identity.

## Scope

Change only the Vibe Kanban frontend logic that derives Git panel repository
rows, plus focused tests and reusable project documentation. No other service
or homelab deployment behavior is changed.

## Requirements

1. A repository row may display a PR only when PR data is associated with that
   repository's `RepoBranchStatus` entry.
2. A PR belonging to one repository must never appear on another repository's
   row, including during initial loading or refresh.
3. Repositories without an associated PR must retain their normal per-repo
   action affordances. Existing commit/change rules remain unchanged.
4. Open PRs remain preferred over merged PRs when both are present for one
   repository, preserving current behavior.
5. Single-repository and multi-repository workspaces use the same safe
   association rule; workspace summary data must not be guessed onto a repo.
6. Focused automated tests must cover mixed multi-repo state and the loading
   state where branch status is not yet available.

## Design

Extract the repo-to-panel transformation into a small pure helper colocated
with `GitPanelContainer`. For each configured repository it finds the matching
`RepoBranchStatus` by `repo_id`, selects that status entry's open PR or merged
PR, and builds `RepoInfo`. If no matching status or merge exists, PR fields stay
undefined.

Remove the workspace-summary PR fallback and its dependency on active/archive
workspace collections. This is deliberate: the summary exposes no `repo_id`,
so applying it to any row would be an unverified association. The branch-status
query is the authoritative repository-scoped source already used after load.

The reported manually-created remote PR that never appeared is not repaired by
guessing from issue-level or workspace-level PR lists. That path requires a
separate explicit remote-repository-to-local-repo association and is outside
this rendering fix.

## Acceptance criteria

- A two-or-more-repo fixture with a PR on exactly one repo produces PR fields
  only on that repo's `RepoInfo`.
- With branch status unavailable, no repo receives workspace summary PR data.
- A repo with both open and merged PR records displays the open PR.
- Existing Git panel type checking, formatting, and relevant tests pass.
- Independent Codex review reports no significant findings.

## Risks and mitigations

- **Temporary blank PR state while branch status loads:** preferable to a
  confidently wrong cross-repo link; the scoped status populates afterward.
- **Manual remote PR discovery remains absent:** explicitly out of scope because
  no trustworthy repo identity is present in the workspace summary fallback.
- **Transformation regressions:** isolate the logic and exercise it with pure
  unit tests rather than relying only on a rendered component test.
