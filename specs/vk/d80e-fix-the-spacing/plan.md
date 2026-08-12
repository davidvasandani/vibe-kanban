# Implementation Plan: Compact Right Drawer Section Spacing

**Spec**: `./spec.md`
**Status**: Ready

## Technical Context

The affected surface is the React/TypeScript workspace drawer in
`packages/web-core`, shared by local and remote frontends and mounted on both
desktop and mobile layouts. Tailwind utility classes provide layout. The shared
`packages/ui/src/components/CollapsibleSectionHeader.tsx` primitive owns live
disclosure state and converts `fillAvailableSpace` into expanded `flex-1
min-h-0` sizing. Vitest with JSDOM provides rendered class-contract coverage.

## Architecture & Approach

1. Add a required `fillAvailableSpace` boolean to the local `SectionDef` model
   in `packages/web-core/src/pages/workspaces/RightSidebar.tsx`.
2. Set the flag to `false` for Server Affinity and `true` for every other
   existing section. Pass the policy into `CollapsibleSectionHeader` through a
   new explicit `intrinsicHeight` mode, preserving the primitive's legacy
   omitted/false `h-full` contract for other callers.
3. Leave `ServerAffinitySectionContainer.tsx` unchanged: its two-column grid is
   already the established responsive layout and will become compact once its
   parent no longer grows.
4. Extend `packages/web-core/src/pages/workspaces/RightSidebar.test.tsx` to
   render the real disclosure primitive, expand affinity, identify section
   roots through their buttons, and assert the affinity root is intrinsic while
   an expanded content section still carries `flex-1 min-h-0`.
5. Extend `CollapsibleSectionHeader.test.tsx` to protect the mutually exclusive
   intrinsic class contract directly.

## Data Model

No persisted or API data changes. `SectionDef.fillAvailableSpace` is transient
view-composition metadata only.

## Contracts

No network or shared public type contract changes. The existing
`CollapsibleSectionHeader.fillAvailableSpace` component prop is reused.

## Research Notes

See `./research.md`. No new dependency is required.

## Constitution Check

- **II Test the contract**: rendered-DOM coverage protects the class-level
  behavior at the drawer composition boundary.
- **III Small, reversible steps**: the implementation changes only per-section
  metadata and a prop value; no primitive or affinity logic is rewritten.
- **IV Shared-component boundaries**: feature-specific sizing remains in
  `web-core`; the reusable `packages/ui` primitive remains generic.
- **VI Don't rebuild what shipped**: the existing `fillAvailableSpace` API and
  Server Affinity grid are reused.
- **XIV Repository verification is worktree-safe**: install prerequisites are
  checked before formatting, and locked setup is used if missing.

No constitution deviations exist.

## Risks & Dependencies

- Omitting the new sizing flag on a section could accidentally change existing
  growth behavior; making the metadata required and covering both policies
  reduces this risk.
- JSDOM cannot prove pixel spacing, so tests protect the Tailwind class contract
  that causes browser layout, supplemented by the supplied screenshot and code
  inspection.
- Full frontend checks depend on the repository's locked pnpm installation.

## Verification

1. Focused `RightSidebar` and `CollapsibleSectionHeader` Vitest tests.
2. `pnpm run web-core:check` and `pnpm run ui:check`.
3. Relevant frontend lint.
4. `pnpm run format` and `git diff --check`.
5. Independent Codex CLI diff review with fixes and re-verification as needed.
