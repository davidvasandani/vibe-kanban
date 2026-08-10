# Default every new workspace to the remote mainline

Task: `vk/1476-protect-git-repo`

## Problem

When Vibe Kanban starts a workspace, one repository-selection path defaults the
target branch to the registered checkout's current local branch. Repositories in
`/srv/src` may be checked out on deployment, recovery, or operator branches, so
that local state is not a safe workspace base. The intended default is the
remote mainline, normally `origin/main`.

## Required behavior

- Every new-workspace repository selection path uses the same default-branch
  policy.
- An explicitly configured repository default remains highest priority.
- Without an explicit default, `origin/main` is preferred, followed by
  `origin/master` for legacy repositories.
- Only when neither remote mainline exists may selection fall back to the
  current branch and then the first available branch.
- An explicit initial branch supplied by the calling workflow remains higher
  priority than repository/default inference when it exists.
- The exact selected remote-tracking branch is persisted as the workspace's
  target branch, so worktree creation resolves that ref rather than local HEAD.
- Empty branch lists remain non-selectable and existing manual overrides remain
  unchanged.

## Scope

This is a Vibe Kanban application change. It must not alter the checkout,
deployment, or branch configuration of any other service under `/srv/src`.

## Verification

- Unit coverage proves configured defaults and explicit initial branches win.
- Coverage proves `origin/main` and `origin/master` outrank a current local
  branch.
- Coverage proves the existing current/first fallback and empty-list behavior.
- Relevant frontend type, lint, format, and test checks pass.
