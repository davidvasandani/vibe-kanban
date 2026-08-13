# Move deployment refresh into Deploy Status

## Problem

The desktop application rail currently renders deployment identity or an
available-deployment `Refresh` control below the user headshot. This mixes
operational deployment controls with account navigation and duplicates the
deployment information already exposed at the top of the workspace right
sidebar.

## Goal

Make the workspace right sidebar's **Deploy Status** section the single desktop
home for deployed revision/age and the page-refresh action that adopts a newly
deployed build. Remove both the git revision and deployment refresh control from
under the AppBar user headshot.

## Functional requirements

1. The desktop AppBar must no longer render the current git revision or the
   deployment `Refresh` control below the user popover/headshot.
2. Native desktop update behavior (`Update` for an installable application
   update) must remain available in the AppBar and retain its existing callback.
3. The workspace right sidebar must present Deploy Status as a collapsible
   accordion/section using the same section-header behavior as the other right
   sidebar sections.
4. The Deploy Status header must continue to show the deployed revision and
   elapsed deployment age when that metadata is available.
5. When a newer web deployment is available, the Deploy Status section must
   expose a `Refresh` action that invokes the existing reload behavior.
6. The refresh action must be absent when no newer deployment is available.
7. Existing workspace sections and their sizing/collapse behavior must remain
   unchanged.
8. Mobile deployment identity remains out of scope and must not regress.

## UX expectations

- Deploy Status appears before the Issue/Git/etc. workspace sections.
- Its header is recognizable as an accordion and can be expanded/collapsed.
- Revision/age remain compact and readable in the section header.
- The available-deployment action is clearly labeled `Refresh` and keyboard
  accessible.
- Clicking the action must not accidentally toggle the accordion.

## Technical scope

- Vibe Kanban source only, primarily `packages/ui` and `packages/web-core`.
- No changes to unrelated homelab services or deployment modules are expected.
- Reuse the existing `deployUpdateAvailable` signal and page reload callback;
  do not add a new backend contract.

## Verification

- Add or update rendered-DOM tests for the AppBar and RightSidebar ownership
  boundary.
- Verify Deploy Status accordion rendering, revision/age display, conditional
  refresh visibility, refresh callback invocation, and non-toggling action
  behavior.
- Run focused frontend tests, type checking/linting appropriate to touched
  packages, formatting, and diff checks.

## Non-goals

- Changing update detection semantics.
- Changing native desktop update installation behavior.
- Modifying deployment infrastructure or any service other than Vibe Kanban.
