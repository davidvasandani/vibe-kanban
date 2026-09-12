# Implementation Plan: Single-Value Browser Titles

**Spec**: `./spec.md`
**Status**: Ready

## Technical Context

- React 18 and TypeScript 5.9 in the shared `@vibe/web-core` frontend package.
- Shared browser-title hook:
  `packages/web-core/src/shared/hooks/usePageTitle.ts`.
- Current issue/project call site:
  `packages/web-core/src/pages/kanban/ProjectKanban.tsx`.
- Existing web-core Vitest 2 lane supports per-file jsdom environments and
  colocated `*.test.tsx` tests.
- No API, persistence, generated type, deployment, or dependency changes.

## Architecture & Approach

1. Preserve `usePageTitle(...parts)` as the single shared title-selection seam
   used by local and remote frontend routes.
2. Reinterpret its arguments as an ordered fallback chain. Select the first
   string containing a non-whitespace character; trim its surrounding
   whitespace and assign it as the whole `document.title`.
3. Use the existing `BASE_TITLE` only if the fallback chain has no meaningful
   candidate.
4. Keep `ProjectKanban`'s `issue?.title, projectName` arguments in that order so
   the issue title wins and the project name remains the loading/no-issue
   fallback. No caller may treat the arguments as fragments after this change.
5. Add a colocated jsdom hook test using React's root renderer and `act` to
   verify initial assignment, fallback selection, whitespace handling, and
   rerender updates.
6. Leave visible navbar breadcrumb and issue-ID rendering untouched.

## Data Model

See `./data-model.md`. No persisted or domain data changes are required.

## Contracts

See `./contracts/browser-title-selection.md`. There are no network/API
contracts.

## Research Notes

See `./research.md`. No new dependency is required.

## Constitution Check

- **I — Clarity over cleverness**: one small selection expression replaces
  string composition in the existing hook.
- **II — Test the contract**: a focused rendered hook test covers selection and
  updates through the actual `document.title` side effect.
- **III — Small, reversible steps**: no routing, domain state, or visible UI
  changes.
- **IV — Shared-component boundaries are law**: cross-frontend metadata behavior
  remains in `packages/web-core`.
- **VI — Don't rebuild what shipped**: the existing shared title hook is
  generalized instead of introducing route-specific effects.
- **VII — Workspace breadcrumbs preserve issue identity**: visible breadcrumbs
  are explicitly unchanged.
- **XIV — Repository verification is worktree-safe**: install locked
  dependencies before validation.
- **XXXI — Browser titles identify one thing**: the approach directly implements
  the ordered single-label contract.

No deviations or constitution violations are expected.

## Risks & Dependencies

- Existing callers may have relied on joined fragments. Source inspection found
  only the issue/project caller with multiple arguments, where ordered fallback
  behavior matches the task intent.
- React effects run after render, so tests must wrap render/rerender in `act` and
  assert after effect flushing.
- The process environment can export `NODE_ENV=production`; invoke tests through
  the package script with `NODE_ENV=test` to keep React's testing build active.

## Verification

- Pre-implementation focused test proves the old concatenation fails.
- Focused `usePageTitle` Vitest after implementation.
- `pnpm --filter @vibe/web-core run check`.
- Repository `pnpm run format` as required by `AGENTS.md`.
- `git diff --check` and scoped diff inspection.
- Independent Codex diff review.
