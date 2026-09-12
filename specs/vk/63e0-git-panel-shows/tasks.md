# Tasks: Repository-scoped Git panel PR links

**Plan**: `./plan.md`

Tasks are dependency ordered. `[P]` tasks touch independent files and may run in
parallel within their layer.

## Phase 1: Association contract

- [x] T001 Extract the repository-to-`RepoInfo` projection and remove the
  workspace-summary PR fallback in `packages/web-core/src/pages/workspaces/gitPanelRepoInfo.ts` and
  `packages/web-core/src/pages/workspaces/GitPanelContainer.tsx`.
- [x] T002 Add mixed multi-repo, unloaded-status, open-precedence, and branch
  metadata unit cases in
  `packages/web-core/src/pages/workspaces/GitPanelContainer.test.ts`
  (depends on T001).

## Phase 2: Verification

- [x] T003 Run locked dependency setup when required, focused Vitest coverage,
  `pnpm run format`, frontend checks, and relevant lint; record results in
  `specs/vk/63e0-git-panel-shows/verification.md` (depends on T001, T002).
- [x] T004 Run independent Codex review and record the clean result and any
  addressed findings in `specs/vk/63e0-git-panel-shows/review.md` (depends on
  T003).

## Phase 3: Knowledge and delivery

- [x] T005 [P] Add the reusable identity-scoped projection invariant to
  `docs/knowledge-base/repository-scoped-ui-projections.md` and refresh
  `docs/knowledge-base/INDEX.md` (depends on T004).
- [x] T006 Commit the completed task artifacts and implementation, push the task
  branch, open a PR, wait for required checks, and merge it (depends on T005).
