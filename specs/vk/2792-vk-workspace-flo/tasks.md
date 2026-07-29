# Tasks: Hide Workspace Context Bar on Mobile Layout

Tasks are ordered by dependency. Tasks marked **[P]** are parallel-safe after
their listed dependencies because they touch independent files or run
independent validation commands.

## Orientation

- [x] T001 Read the feature inputs and confirm the invariant scope: responsive
      mobile layout hides the workspace context bar, physical mobile remains a
      secondary hiding signal, desktop behavior is unchanged, and no API,
      database, generated type, persistence, breakpoint, or `packages/ui`
      contract changes are allowed.
      Files: `specs/vk/2792-vk-workspace-flo/spec.md`,
      `specs/vk/2792-vk-workspace-flo/clarifications.md`,
      `specs/vk/2792-vk-workspace-flo/plan.md`,
      `specs/vk/2792-vk-workspace-flo/research.md`,
      `specs/vk/2792-vk-workspace-flo/data-model.md`,
      `specs/vk/2792-vk-workspace-flo/contracts.md`,
      `specs/vk/2792-vk-workspace-flo/tasks.md`,
      `assets/speckit/memory/constitution.md`.
- [x] T002 Inspect the current workspace composition and mobile layout branch:
      verify `WorkspacesLayout` uses `useIsMobile()`, mobile navigation exposes
      the overlapping destinations, and `WorkspacesMainContainer.hideContextBar`
      remains an explicit composition override.
      Files: `packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx`,
      `packages/web-core/src/pages/workspaces/WorkspacesMainContainer.tsx`.
      Depends on T001.
- [x] T003 Inspect the existing context-bar container and responsive/platform
      hooks before editing: confirm `ContextBarContainer` currently gates only
      on `isRealMobileDevice()`, owns action/position/drag wiring, and
      `useIsMobile()` keeps the existing `(max-width: 767px)` breakpoint.
      Files: `packages/web-core/src/pages/workspaces/ContextBarContainer.tsx`,
      `packages/web-core/src/shared/hooks/useIsMobile.ts`.
      Depends on T001.
- [x] T004 [P] Inspect the presentational context bar and confirm no responsive
      or physical-device awareness should be added to `packages/ui`.
      File: `packages/ui/src/components/ContextBar.tsx`.
      Depends on T001.

## Policy Helper

- [x] T005 Add an implementation-private pure helper near the workspace
      context-bar container, named for the workspace context-bar render policy
      such as `shouldRenderWorkspaceContextBar`.
      File: `packages/web-core/src/pages/workspaces/ContextBarContainer.tsx`
      or a nearby workspace context-bar helper file.
      Depends on T003 and T004.
- [x] T006 Implement the helper truth table: return `false` when
      `isResponsiveMobile` is true, return `false` when `isRealMobileDevice` is
      true, and return `true` only when both signals are false. Keep the helper
      private to `packages/web-core` and do not introduce generated or persisted
      types.
      File: same file chosen in T005.
      Depends on T005.

## Container Wiring

- [x] T007 Wire the policy at the container/composition boundary by having
      `ContextBarContainer` read `useIsMobile()` and combine it with
      `isRealMobileDevice()` through the helper.
      File: `packages/web-core/src/pages/workspaces/ContextBarContainer.tsx`.
      Depends on T006.
- [x] T008 Preserve React hook ordering and desktop behavior while adding the
      gate: read hooks consistently or split into a thin gate plus desktop-only
      inner implementation; keep action filtering, render item mapping,
      `useContextBarPosition(containerRef)`, mouse drag handling, snap behavior,
      IDE/copy special items, and return shape unchanged for desktop.
      File: `packages/web-core/src/pages/workspaces/ContextBarContainer.tsx`.
      Depends on T007.
- [x] T009 Confirm `WorkspacesMainContainer.hideContextBar` remains unchanged
      and no replacement floating mobile shortcut palette is added.
      Files: `packages/web-core/src/pages/workspaces/WorkspacesMainContainer.tsx`,
      `packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx`.
      Depends on T008.

## Automated Coverage

- [x] T010 Add focused Vitest coverage for the pure visibility policy:
      responsive mobile true / physical mobile false hides, responsive mobile
      false / physical mobile true hides, both true hides, and both false
      renders.
      File: `packages/web-core/src/pages/workspaces/ContextBarContainer.test.tsx`
      or the nearby helper test matching the helper location.
      Depends on T006.
- [x] T011 Add a lightweight component-level smoke test only if the wiring is
      not sufficiently protected by the helper test and diff review. If added,
      verify the responsive-mobile/physical-non-mobile mismatch does not mount
      the context bar without over-mocking unrelated action behavior.
      File: `packages/web-core/src/pages/workspaces/ContextBarContainer.test.tsx`
      or a nearby workspace test file.
      Depends on T008 and T010.
- [x] T012 Review existing mobile navigation coverage or manually inspect the
      mobile branch to confirm this feature relies on existing mobile tabs for
      workspace, chat, changes, logs, preview, browser, and Git rather than new
      floating controls.
      File: `packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx`.
      Depends on T009.

## Verification

- [x] T013 [P] Run the focused web-core test suite, preferably the new/affected
      Vitest file first and then `pnpm --filter @vibe/web-core run test`.
      Depends on T010, and on T011 if the optional smoke test is added.
- [x] T014 [P] Run frontend type checking, using the repository's available
      web-core lane if present or full `pnpm run check`.
      Depends on T008 and T010.
- [ ] T015 [P] Perform manual responsive verification: at `<= 767px` on a
      desktop browser, confirm the workspace chat does not mount or display the
      floating context bar while mobile navigation remains available; at desktop
      viewport on a non-mobile device, confirm the context bar renders.
      Depends on T008, T009, and T012.
- [x] T016 Run `pnpm run lint` after implementation and focused checks are in
      place.
      Depends on T013 and T014.
- [x] T017 Run repository formatting with `pnpm run format`.
      Depends on T016.
- [x] T018 Run `git diff --check`.
      Depends on T017.

## Final Review

- [x] T019 Inspect the final diff against the contracts: no generated files,
      SQLx migrations, backend routes, persistence schemas, mobile breakpoint,
      context-bar actions, routes, `packages/ui` context-bar API, or snap
      position persistence were changed.
      Depends on T018.
- [x] T020 Run a final acceptance review covering all required cases: mobile
      responsive layout hidden even when physical mobile is false; physical
      mobile hidden even when responsive desktop is possible; desktop non-mobile
      visible with existing action set; desktop drag, snap, and persisted snap
      behavior unchanged; existing mobile navigation still functional.
      Depends on T019.
- [x] T021 Record validation results and any residual manual-verification gaps
      in the implementation handoff or PR notes.
      Depends on T020.

## Parallel Notes

T004 can run in parallel with T002 and T003 after T001 because it is read-only
and targets the independent presentational `packages/ui` component. T013, T014,
and T015 can run in parallel once their dependencies are satisfied because they
exercise independent validation lanes. Formatting, diff checks, and final review
should run after those validation tasks complete.

## Implementation results

- Focused truth-table tests: 4 passed.
- Full `@vibe/web-core` suite: 214 tests passed.
- `@vibe/web-core` type check: passed.
- Frontend ESLint (`local-web`, `ui`), backend Clippy (main and remote
  workspaces), and unused-i18n-key checks: passed.
- Repository formatting and `git diff --check`: passed.
- T015 remains a manual verification gap because this task environment has no
  configured browser/touch runner. The code-level acceptance review confirmed
  the mobile layout uses the tested responsive signal and the desktop
  positioning/drag implementation is unchanged.
