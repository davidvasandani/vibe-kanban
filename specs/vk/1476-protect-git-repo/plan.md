# Implementation Plan: Remote-mainline workspace defaults

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical Context

React/TypeScript branch-selection state lives in `packages/web-core`. The pure
`resolveDefaultBranch` helper already implements the desired policy and is used
by the primary create-mode picker. `useRepoBranchSelection` duplicates a
different current-first policy and is the remaining reusable seam to reconcile.

## Architecture & Approach

Import `resolveDefaultBranch` into `useRepoBranchSelection`. Preserve a valid
`initialBranch` check first, then use the helper for repository default and all
fallback inference. Keep `userOverrides` above both. Add a focused rendered-hook
test with mocked branch queries, including the exact workspace input emitted.

No backend change is needed: the backend already resolves remote-prefixed target
branches and clustered stores materialize those refs.

## Constitution Check

Principles II, III, IV, and VI are satisfied by reusing the existing pure helper,
testing the selection contract in `web-core`, and avoiding new plumbing. No
deviation is required.

## Risks

The hook is currently lightly used/dormant, so tests must exercise its public
output rather than relying only on the helper's existing unit tests.
