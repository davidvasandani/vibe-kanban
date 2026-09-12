# Implementation Plan: Expand Mobile Workspace Toolbar

**Spec**: `./spec.md`
**Status**: Ready

## Technical Context

- React 18 and TypeScript 5.9 shared frontend.
- Tailwind utility classes, scoped through the existing new-design system.
- Presentational navbar: `packages/ui/src/components/Navbar.tsx`.
- Container/state composition: `packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx`.
- Existing rendered-DOM Vitest lane:
  `packages/web-core/src/shared/components/ui-new/Navbar.mobile.test.tsx`.
- No API, persistence, generated type, deployment, or dependency changes.

## Architecture & Approach

The mobile navbar already separates workspace/project leading content from a
trailing status/action group. Retain that composition and change only the
workspace branch of the shared presentational component:

1. Give the non-project leading region `flex-1 min-w-0` so it owns all room left
   by the `shrink-0` trailing group and can shrink without moving that group.
2. Keep horizontal overflow on the leading region.
3. Give the tab-group wrapper full/minimum width and give each visible tab equal
   flexible growth plus a usable minimum width. This shares surplus space while
   allowing the intrinsic minimum total to overflow and scroll.
4. Keep leading Back/Projects navigation and Board as intrinsic-width controls;
   only workspace tool tabs share tab-strip surplus.
5. Add stable test selectors to the layout regions if semantic attributes alone
   cannot select the sizing boundaries reliably.
6. Extend the existing real-`Navbar` rendered-DOM tests to assert the exact flex
   contract and preserve the active `aria-pressed` behavior.

## Data Model

See `./data-model.md`. No domain or persisted data changes are required.

## Contracts

See `./contracts/mobile-toolbar-layout.md`. There are no network/API contracts.

## Research Notes

See `./research.md`.

## Constitution Check

- **I — Clarity over cleverness**: the solution is a localized flexbox contract
  using existing utilities.
- **II — Test the contract**: the real shared component is rendered in the
  existing web-core Vitest lane and its layout/accessibility contract asserted.
- **III — Small, reversible steps**: no state, routes, APIs, or dependencies
  change.
- **IV — Shared-component boundaries are law**: internal navbar layout remains
  owned by `packages/ui`; `packages/web-core` supplies the established test lane.
- **VI — Don't rebuild what shipped**: the existing mobile navbar and test file
  are extended.
- **XIV — Repository verification is worktree-safe**: locked dependencies are
  installed before checks; repository format runs before completion.

No deviations or constitution violations are expected.

## Risks & Dependencies

- A flexible tab with no minimum can become an unusably small tap target. Use a
  minimum width and let the strip scroll below that threshold.
- `overflow-x-auto` can fail inside flex layout if the region retains its
  automatic minimum width. Keep `min-w-0` on the flexible owner.
- Labels appear only at 480px and above; equal distribution must work with and
  without label text.
- Changes affect both local and remote frontends because `packages/ui` is shared;
  typecheck the shared UI and web-core consumers.

## Verification

- Focused navbar Vitest.
- `pnpm --filter @vibe/ui check` and lint.
- `pnpm --filter @vibe/web-core check`.
- Repository `pnpm run format`.
- `git diff --check` for authored files.
- Independent Codex diff review.
