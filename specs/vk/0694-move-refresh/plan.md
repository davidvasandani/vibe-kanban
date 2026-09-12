# Implementation Plan: Move deployment refresh

**Spec**: `./spec.md`
**Status**: Ready

## Technical Context

The change is confined to the React/TypeScript shared frontend. `packages/ui`
owns the AppBar and collapsible-section presentation primitives;
`packages/web-core` owns deployment-update detection, workspace layout data
flow, and feature-level rendered-DOM tests. The existing deployment metadata
comes from `useUserSystem`; update detection comes from
`useDeployUpdateAvailable` and reload is the browser's `window.location.reload`.

## Architecture & Approach

1. In `packages/ui/src/components/AppBar.tsx`, narrow the bottom deployment
   branch to native `updateVersion` handling only. Retain the native Update
   callback and remove the web-deployment Refresh/current-revision branches.
2. In `packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx`, consume the
   existing `useDeployUpdateAvailable` hook for the workspace route and pass its
   boolean plus the page reload callback into `RightSidebar`.
3. In `packages/web-core/src/pages/workspaces/RightSidebar.tsx`, model Deploy
   Status as the first `SectionDef`, with compact `DeployStatus` metadata in
   `headerExtra`, intrinsic sizing, and a conditional refresh header action.
4. Extend `SectionAction` only as needed to give the refresh action an explicit
   accessible/user-facing label while preserving stop-propagation behavior in
   `packages/ui/src/components/CollapsibleSectionHeader.tsx`.
5. Add a Deploy Status persistence key to
   `useUiPreferencesStore.ts` and its `PersistKey` union.
6. Update `RightSidebar.test.tsx` and existing AppBar consumer tests to cover
   ownership, conditional rendering, interaction isolation, and native Update.

## Data Model

See `./data-model.md`. No persisted backend entities or generated contracts
change.

## Contracts

See `./contracts/ui-contracts.md`. All contracts are component props and UI
behavior; there is no API/schema change.

## Research Notes

See `./research.md` for the ownership and event-handling decisions.

## Constitution Check

- **I Clarity over cleverness**: reuse the existing update hook, DeployStatus,
  and section primitive with direct props.
- **II Test the contract**: rendered-DOM tests cover the ownership and click
  boundary.
- **III Small, reversible steps**: no backend, update-detection, or deployment
  infrastructure changes.
- **IV Shared-component boundaries are law**: UI owns primitive rendering;
  web-core owns data/hook consumption and feature tests.
- **VI Don't rebuild what shipped**: existing metadata, detection, reload, and
  collapse mechanisms are reused.
- **XIV Repository verification is worktree-safe**: locked dependencies are
  installed before mandated verification.

No constitution deviations are required.

## Risks & Dependencies

- A header action is nested within the section disclosure button's content.
  The existing action primitive uses a focusable role-button and stops click and
  keyboard propagation; tests must prove refresh does not toggle disclosure.
- Duplicate consumers of `useDeployUpdateAvailable` share a TanStack Query and
  module-level boot revision, so they do not create independent detection
  semantics. The AppBar consumer may later be removable if no other layout
  branch needs it, but that cleanup is not required to ship this relocation.
- Deploy Status has no meaningful body today. An empty expanded body must not
  claim flexible height; intrinsic sizing prevents it from consuming the
  workspace drawer.
