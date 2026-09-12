# Implementation Plan: Discoverable mobile workspace right drawer

**Spec**: `./spec.md`
**Status**: Ready

## Technical Context

The feature is a React/TypeScript change spanning the shared presentational
navbar in `packages/ui/src/components/Navbar.tsx` and the stateful local
container in
`packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx`.
`packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx` already routes the
persisted `git` mobile tab to `RightSidebar`; that identifier must remain stable.
Testing uses the existing web-core Vitest/jsdom setup. No backend, generated
types, storage schema, dependency, or deployment change is required.

## Architecture & Approach

1. Extend the shared mobile-tab descriptor with an optional accessible label.
   Change only the existing `git` descriptor's presentation to a mirrored
   sidebar icon, visible label `Sidebar`, and accessible label `Right sidebar`.
2. Render each mobile tab with `aria-label` and `aria-selected`, preserving the
   existing click callback and horizontally scrollable strip.
3. Derive local mobile tabs in `NavbarContainer` from `MOBILE_TABS`, omitting
   `git` when there is no selected workspace or create mode is active. Pass the
   derived list through the existing `mobileTabs` prop.
4. When the current tab is `git` and that destination becomes unavailable,
   move mobile layout state to `chat` in create mode or `workspaces` on the
   workspace-less landing so hidden navigation never leaves an empty surface.
5. Keep `MobileTabId`, `useMobileActiveTab`, and the `WorkspacesLayout` `git`
   branch unchanged. This preserves stored values and reuses the existing
   `RightSidebar` mount without adding state or composition.
6. Add a rendered-DOM test in web-core that verifies the shared navbar's drawer
   control semantics, active state, and click callback, plus a pure container
   tab-selection helper test for availability states if extracting the helper
   materially simplifies coverage.

## Data Model

See `./data-model.md`. There is no persistent model migration.

## Contracts

See `./contracts.md` for the shared mobile-tab UI contract. No network contract
changes.

## Research Notes

See `./research.md`.

## Constitution Check

- Principle II: rendered-DOM coverage tests the user-visible accessibility and
  activation contract.
- Principle III and VI: the plan reuses the existing `git` destination and
  `RightSidebar` mount; it adds no overlay, store, or duplicated content.
- Principle IV: tab presentation remains in `packages/ui`; workspace-dependent
  filtering remains in the `web-core` container.
- Constraint on dependencies: no dependency is added.
- Formatting: `pnpm run format` is included in verification.

No deviations or unresolved questions remain.

## Risks & Dependencies

- `MOBILE_TABS` is shared with remote-web. Its clearer presentation should
  apply there too, while local availability filtering must not accidentally
  remove the remote workspace destination.
- The tab strip is space-constrained. Existing `overflow-x-auto` and hidden
  labels below 480px are retained, and the accessible label carries meaning at
  narrow widths.
- The availability fallback changes only transient active layout state; the
  stable identifier remains accepted and requires no preference migration.

