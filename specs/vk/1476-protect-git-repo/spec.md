# Feature Specification: Remote-mainline workspace defaults

**Feature dir**: `specs/vk/1476-protect-git-repo/`
**Status**: Implemented

## Summary

Make every reusable Vibe Kanban repository-selection seam default a new
workspace to the remote mainline instead of the registered checkout's current
local branch. This protects all registered repositories without mutating their
source checkouts.

## User Stories

- As a user, I want a new workspace to start from `origin/main` even when the
  registered checkout is on a deployment or feature branch.
- As an operator, I want repository checkouts under `/srv/src` to remain
  independent from workspace-base inference.

## Functional Requirements

- FR-1: An explicit valid initial branch MUST remain the highest-priority input.
- FR-2: A configured repository default MUST outrank built-in inference.
- FR-3: Otherwise `origin/main`, then `origin/master`, MUST outrank the current
  local branch.
- FR-4: Current branch and first available branch MUST remain fallbacks.
- FR-5: The exact remote-prefixed branch name MUST be emitted in workspace repo
  inputs.
- FR-6: Manual per-repository overrides and empty branch-list behavior MUST be
  preserved.
- FR-7: The change MUST NOT mutate registered repositories or any other service.

## Acceptance Criteria

- [x] A current local deployment branch plus `origin/main` selects
      `origin/main`.
- [x] Legacy repos select `origin/master` when `origin/main` is absent.
- [x] Explicit initial and configured defaults retain their precedence.
- [x] Fallback and empty-list behavior remain covered.
- [x] Focused tests, formatting, type checking, and linting pass.

## Open Questions

None. The existing canonical helper and knowledge page define the policy.
