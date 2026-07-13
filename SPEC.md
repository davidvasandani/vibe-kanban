# Technical Spec: Default repo branch to `origin/main` when creating issues

> Task c59f. Full SpecKit artifacts live in
> `homelab/specs/vk/c59f-default-to-origi/` (`spec.md`, `plan.md`, `research.md`,
> `data-model.md`, `tasks.md`). This file is the repo-root technical summary.

## Problem

On the create-issue screen ("Which repositories would you like to work on?"),
adding a repository forced the user through a modal branch picker and applied no
default — the row read "Select branch" until the user chose one. The desired
behavior (per the mock) is that each repository defaults to `origin/main`
immediately on add, with no forced pick, while the branch can still be changed
afterward.

## Solution

Frontend-only change in `packages/web-core`, in two additive pieces.

### 1. Pure resolver (`packages/web-core/src/shared/lib/defaultBranch.ts`)

New `resolveDefaultBranch(branches: GitBranch[], preferredBranch?: string |
null): string | null`. Returns the first branch (matched by exact `name`) that
exists, in priority order:

1. `preferredBranch` — the repo's configured `default_target_branch`, so an
   explicit user choice outranks the built-in default.
2. `origin/main`, then `origin/master` (`DEFAULT_BRANCH_PREFERENCE`) — the
   requested default plus the main/master naming fallback.
3. the current branch (`is_current`).
4. the first branch.

Returns `null` only when `branches` is empty. Pure and dependency-free.

### 2. Wire into the add path (`CreateModeRepoPickerBar.tsx`)

`addRepoWithBranchSelection` no longer opens the forced branch modal. It now
fetches the repo's branches (`repoApi.getBranches`), computes
`resolveDefaultBranch(branches, repo.default_target_branch)`, and — when
non-null — adds the repo and sets that branch. An empty branch list surfaces a
picker error and the repo is not added (preserving the "every repo has a branch
before submit" guard). `pickBranchForRepo` is retained, still used by the
"Change branch" button so the default remains overridable.

## Why not elsewhere

- **Backend `git::get_all_branches` sort** — the frontend selects by exact name,
  so a backend sort would not set the default and would enlarge a hot upstream
  file's diff. Left untouched (frontend-only keeps the fork mergeable).
- **The dormant `useRepoBranchSelection.ts` / `RepoBranchSelector.tsx`** — no
  importers; its fallback prefers the current branch. Not wired into this
  screen; left as-is and noted as a future reconciliation.

## Scope

- No Rust, API, schema, migration, or generated-type changes. `GitBranch`,
  `Repo.default_target_branch`, and the request types already exist.
- Interactive add only; bootstrapped/linked-issue repos already carry a saved
  `target_branch`.

## Validation

- New unit tests `defaultBranch.test.ts` cover each ordered case (origin/main,
  origin/master fallback, configured-default precedence, ignore-stale-configured,
  current/first fallback, empty → null). `pnpm --filter @vibe/web-core test`
  passes (104 tests).
- `pnpm --filter @vibe/web-core run check` (tsc) passes; Prettier clean.

## Files

- `packages/web-core/src/shared/lib/defaultBranch.ts` (new)
- `packages/web-core/src/shared/lib/defaultBranch.test.ts` (new)
- `packages/web-core/src/shared/components/CreateModeRepoPickerBar.tsx` (edited)
