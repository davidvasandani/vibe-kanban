# Implementation Plan: Default repo branch to `origin/main` (task vk/c59f-default-to-origi)

Step-by-step build order. The authoritative dependency-ordered task list is
`homelab/specs/vk/c59f-default-to-origi/tasks.md`; this is the repo-root summary.

## Goal

On the create-issue repo picker, default each added repository's target branch
to `origin/main` (with sensible fallbacks) instead of forcing a modal branch
pick. Frontend-only, in `packages/web-core`.

## Steps

1. **Add the pure resolver** — `packages/web-core/src/shared/lib/defaultBranch.ts`.
   Export `DEFAULT_BRANCH_PREFERENCE = ['origin/main', 'origin/master']` and
   `resolveDefaultBranch(branches, preferredBranch?)`. Priority: configured
   `preferredBranch` → `origin/main` → `origin/master` → `is_current` → first →
   `null`. Match by exact `GitBranch.name`. (T001)

2. **Wire into the add path** — `CreateModeRepoPickerBar.tsx`.
   In `addRepoWithBranchSelection`, replace the forced `pickBranchForRepo` call
   with: `const branches = await repoApi.getBranches(repo.id)` →
   `resolveDefaultBranch(branches, repo.default_target_branch)`. If `null`, set a
   picker error and return `false` (don't add). Otherwise `addRepo(repo)` +
   `setTargetBranch(repo.id, defaultBranch)`. Drop `pickBranchForRepo` from that
   callback's deps but keep the function — `handleChangeBranch` still uses it for
   the "Change branch" button. Add the `resolveDefaultBranch` import. (T002)

3. **Unit tests** — `packages/web-core/src/shared/lib/defaultBranch.test.ts`.
   Cover: empty → null; origin/main preferred; origin/master fallback;
   configured-default precedence; stale configured-default ignored; current
   fallback; first-branch fallback; null/empty configured default ignored. (T003)

4. **Validate** — `pnpm --filter @vibe/web-core test` (all pass),
   `pnpm --filter @vibe/web-core run check` (tsc), Prettier clean. No
   `generate-types` / `prepare-db` (no Rust/type change). (T004)

## Non-goals / deliberately untouched

- Backend `git::get_all_branches` (sort/order) — frontend selects by name.
- Dormant `useRepoBranchSelection.ts` / `RepoBranchSelector.tsx` (no importers).
- Bootstrapped/linked-issue repos (already carry a saved `target_branch`).
- Persisting `origin/main` as a repo's stored `default_target_branch`.

## Status

All steps complete. `defaultBranch.ts`, its test, and the
`CreateModeRepoPickerBar.tsx` edit are in place; web-core tests (104) and tsc
pass. Independent Codex review of the diff is the next stage.
