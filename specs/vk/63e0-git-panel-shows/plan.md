# Implementation Plan: Repository-scoped Git panel PR links

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical Context

- React and TypeScript in `packages/web-core` supply data to the presentational
  `packages/ui` Git panel.
- `useBranchStatus(workspaceId)` returns `RepoBranchStatus[]`; each entry has a
  `repo_id` and repository-owned `merges`.
- `Workspace` summaries expose one optional PR but no repository identity.
- Vitest is the existing frontend unit-test runner. No dependency or generated
  type change is required.

## Architecture & Approach

1. In `packages/web-core/src/pages/workspaces/gitPanelRepoInfo.ts`, extract a
   pure exported transformation, used by `GitPanelContainer.tsx`, that maps configured repositories to
   `RepoInfo` by matching `RepoBranchStatus.repo_id` to repository `id`.
2. Select PR information only from the matched status entry's `merges`, keeping
   the current open-before-merged precedence.
3. Remove `useWorkspaceContext` and the workspace-summary fallback. An absent
   match produces undefined PR fields while retaining the existing branch and
   commit defaults.
4. Add `GitPanelContainer.test.ts` beside the container to prove mixed sibling
   state, unloaded status, and per-repo PR precedence without mounting provider
   infrastructure.

## Data Model

See `./data-model.md`. This task changes no durable schema.

## Contracts

No API or external contract changes. The internal projection contract is:
`repo.id === status.repo_id` is required before any `status.merges` PR can be
shown on that repo.

## Research Notes

See `./research.md`. No new dependency is introduced.

## Constitution Check

- Principle II: focused pure-helper tests exercise the user-visible association
  contract.
- Principles III and VI: reuse the existing branch-status source and make the
  smallest removal of an unsafe fallback.
- Principle IV: data association stays in `web-core`; `packages/ui` remains
  presentational and unchanged.
- Principle XXXIV: configured repository rows remain usable while enrichment
  loads.
- Principle XXXV: only matching entity identity permits projection.

No deviations.

## Risks & Dependencies

- PR information is temporarily absent until branch status resolves. This is a
  truthful loading state and is less harmful than a false cross-repository link.
- Manually-created, unlinked remote PRs remain undiscovered. Repair requires an
  explicit identity-bearing association outside this focused rendering fix.
