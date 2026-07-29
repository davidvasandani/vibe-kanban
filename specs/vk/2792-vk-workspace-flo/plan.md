# Technical Plan: Hide Workspace Context Bar on Mobile Layout

**Feature dir**: `specs/vk/2792-vk-workspace-flo/`
**Task**: `vk/2792-vk-workspace-flo`
**Spec**: [`spec.md`](spec.md)

## Approach

Hide the workspace context bar at the workspace composition or context-bar
container boundary by combining the existing responsive layout signal with the
existing physical-device signal.

The mobile workspace layout is already selected by `useIsMobile()` in
`WorkspacesLayout`. The context bar currently suppresses itself only when
`isRealMobileDevice()` is true, which leaves a gap when the responsive layout is
mobile but physical-device detection is false. The implementation should close
that gap without changing the presentational `@vibe/ui` context bar, action
definitions, routing, persisted snap position, or breakpoint.

Expected implementation shape:

- introduce a small pure visibility predicate in `packages/web-core`, near the
  workspace context-bar container;
- have `ContextBarContainer` read `useIsMobile()` unconditionally and return
  `null` when either responsive mobile or real-device mobile is true;
- keep `WorkspacesMainContainer.hideContextBar` unchanged for carousel and
  other explicit suppression contexts;
- keep `packages/ui/src/components/ContextBar.tsx` layout-agnostic.

No backend, database, API, generated TypeScript, or persistence schema change is
expected.

## Grounding

- `packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx`
  - reads `useIsMobile()` and branches into the mobile workspace composition;
  - mobile tabs already expose workspaces, chat, changes, logs, preview,
    browser, and Git destinations;
  - mobile tab switching uses `hidden` CSS classes to preserve state.
- `packages/web-core/src/pages/workspaces/WorkspacesMainContainer.tsx`
  - mounts `ContextBarContainer` through `contextBarContent` only when a
    workspace/session exists and `hideContextBar` is false;
  - already treats the context bar as optional chrome for carousel contexts.
- `packages/web-core/src/pages/workspaces/ContextBarContainer.tsx`
  - prepares context-bar action state, render items, desktop position, and drag
    handlers;
  - currently returns `null` only when `isRealMobileDevice()` is true.
- `packages/web-core/src/shared/hooks/useIsMobile.ts`
  - defines the canonical responsive mobile signal with `(max-width: 767px)`;
  - exposes `isRealMobileDevice()` and `useIsRealMobile()` for physical-device
    detection.
- `packages/ui/src/components/ContextBar.tsx`
  - remains presentational and should not import responsive or platform hooks.

## Implementation Steps

1. Establish baseline and inspect current test behavior.
   - Confirm the current context-bar container path and mobile layout branch.
   - Run the focused frontend tests only if useful before editing:
     `pnpm --filter @vibe/web-core test`.
2. Add a pure visibility predicate in `packages/web-core`.
   - Suggested name:
     `shouldRenderWorkspaceContextBar({ isResponsiveMobile, isRealMobileDevice })`.
   - Return `false` when either signal is true.
   - Return `true` only when both signals are false.
   - Keep this predicate implementation-only; do not export it from shared
     generated types.
3. Wire the predicate at the container/composition boundary.
   - Preferred minimal option: update `ContextBarContainer` to call
     `useIsMobile()` and combine it with `isRealMobileDevice()`.
   - Read hooks unconditionally before any early return so React hook ordering
     remains stable.
   - Preserve all existing action filtering, render item mapping,
     `useContextBarPosition(containerRef)`, drag handlers, and desktop return
     shape.
   - If hook-order clarity is better served by avoiding expensive work before
     the mobile return, split the component into a thin gate component and an
     inner desktop-only implementation. Keep the public prop shape unchanged.
4. Keep explicit suppression behavior intact.
   - Do not remove or reinterpret `WorkspacesMainContainer.hideContextBar`.
   - Do not add a replacement floating mobile shortcut palette.
5. Add focused automated coverage.
   - Unit-test the pure predicate for:
     responsive mobile true / real mobile false -> hidden;
     responsive mobile false / real mobile true -> hidden;
     both true -> hidden;
     both false -> visible.
   - Add a lightweight component-level test only if the predicate test does not
     sufficiently protect the wiring decision.
   - Keep tests in `packages/web-core/src/pages/workspaces/` or a nearby
     workspace model/lib location, matching existing Vitest style.
6. Perform manual responsive verification.
   - At a viewport <= 767px on a desktop browser, confirm workspace chat does
     not mount/display the context bar.
   - At a desktop viewport on a non-mobile device, confirm the context bar still
     renders and can be dragged/snapped.
   - Confirm mobile tabs for changes, logs, preview, browser, and Git remain
     visible and functional.
7. Run validation before handoff.
   - `pnpm --filter @vibe/web-core test`
   - `pnpm run check`
   - `pnpm run lint`
   - `pnpm run format`
   - `git diff --check`
8. Review the diff.
   - Confirm no generated files were edited manually.
   - Confirm no mobile breakpoint changed.
   - Confirm no action definition, route, persistence key, or `packages/ui`
     presentational context-bar API changed.

## Contracts

See [`contracts.md`](contracts.md). This feature changes only frontend
visibility behavior. It does not add or alter HTTP, database, generated type, or
persistence contracts.

## Data Model

See [`data-model.md`](data-model.md). There is no persistent data model change.
The only useful structured model is an implementation-private boolean policy
input for tests and readability.

## Constitution Check

- **I. Clarity over cleverness**: use a direct boolean visibility rule at the
  existing workspace chrome boundary.
- **II. Test the contract**: predicate tests must cover disagreement between
  responsive-layout and physical-device detection.
- **III. Small, reversible steps**: the expected change is scoped to
  `packages/web-core` context-bar visibility and focused tests.
- **IV. One MCP contract for all agents**: not applicable; this feature does
  not alter MCP configuration or agent launch behavior.
- **V. Settings host scope is a data boundary**: not applicable; this feature
  does not touch Settings reads, writes, cache keys, draft state, or host-scoped
  data.
- **VI. Responsive layout state owns layout chrome**: `useIsMobile()` is the
  visibility authority for mobile layout; physical-device detection remains
  defense in depth and cannot contradict the selected mobile layout.

## Risks

- Reading `useIsMobile()` after an early return would violate hook-order
  expectations if future edits add more returns; read hooks consistently.
- Returning before `useContextBarPosition(containerRef)` may skip desktop-only
  position setup on mobile. That is intended only if hook structure is split
  cleanly; avoid conditional hook calls inside a single component.
- Component-level tests for `ContextBarContainer` may require substantial hook
  mocking because it depends on action context, user system config, and desktop
  position state. A pure predicate plus one integration-style smoke test is a
  better risk-scaled target.
- A CSS-only hide would still mount desktop chrome and drag handlers. The spec
  requires the bar not be mounted or visible on mobile, so prefer conditional
  rendering.

## Rollback

Revert the new predicate, the `ContextBarContainer` visibility gate, and the
focused tests. Since API shape, persistence, generated types, and shared UI
component props are unchanged, rollback is limited to frontend source and tests
under `packages/web-core`.
