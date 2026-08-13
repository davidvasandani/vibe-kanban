# Technical Specification: Commits Behind in the Git Header

## Problem

The workspace right sidebar exposes per-repository branch divergence only inside
the expanded Git panel. A user looking at the collapsed Git section cannot see
whether the current task branch has fallen behind its configured target branch
(normally `main`). This makes stale branches easy to overlook.

## Scope

Change only the Vibe Kanban service repository. No other service or homelab
deployment changes are required.

## Desired behavior

1. The `Git` collapsible-section header displays the current workspace branch's
   non-zero commits-behind count, using the existing branch-status data and each
   repository's configured target branch.
2. For a workspace with one repository, the header shows the commits-behind
   count without redundantly showing the repository name.
3. For a workspace with multiple repositories, every repository with a non-zero
   commits-behind count is identified by its display name and count so the
   values cannot be confused.
4. Repositories that are not behind are omitted. When no repository is behind,
   the Git header remains visually unchanged.
5. The indicator remains available while the Git body is collapsed and updates
   when branch-status query data changes or the selected workspace changes.
6. The header remains compact: overflow is truncated and the full status is
   available as accessible/title text.

## Existing system seam

- `RightSidebar.tsx` owns the `Git` section header and already supports a
  `headerExtra` node.
- `useBranchStatus(workspaceId)` supplies `RepoBranchStatus[]`, including
  `repo_id` and `commits_behind` calculated against the configured target.
- `RepoWithTargetBranch` supplies stable repository identity and display names.
- `GitPanelContainer.tsx` already consumes the same status fields for the
  expanded repository cards; the header indicator must preserve those
  semantics and avoid introducing a second backend contract.

## Implementation constraints

- Do not fetch a remote provider directly or introduce a new polling path.
- Do not sum multiple repositories into an ambiguous project-wide number.
- Treat absent/loading status and absent `commits_behind` values as no indicator,
  rather than briefly displaying a misleading zero.
- Preserve existing Git panel actions, sizing, persistence, and per-repository
  ahead/behind indicators.
- Add focused frontend tests for zero, single-repository, and multi-repository
  presentation and for placement in the Git section header.

## Acceptance criteria

- A single repository 3 commits behind its target shows a compact `3 behind`
  indicator in the Git header.
- With repositories `web` 2 behind and `server` 5 behind, the header identifies
  both values (for example, `web 2 · server 5`).
- A repository at zero behind produces no Git-header status.
- Counts are based on repository IDs, so status remains correct regardless of
  input ordering.
- Relevant frontend tests, formatting, type checks, and linting pass.

## Non-goals

- Changing how divergence is calculated by Git.
- Fetching target branches or remotes more frequently.
- Showing commits ahead, unpushed commits, PR state, or rebase controls in the
  section header.
- Modifying deployment or any service outside Vibe Kanban.
