# Research: Move deployment refresh

## Existing ownership

- `AppBar.tsx` currently uses one conditional chain for native Update, newer-web
  Refresh, and current revision. Only the latter two move.
- `RightSidebar.tsx` already reads revision/timestamp and renders `DeployStatus`
  in a fixed row.
- `CollapsibleSectionHeader` already provides independent section actions with
  mouse and keyboard propagation stopped before invoking the action.
- `useDeployUpdateAvailable` freezes the first observed revision at module scope
  and polls through a shared TanStack Query key.

## Decisions

### Reuse the update detector in the workspace route

Consuming `useDeployUpdateAvailable` from `WorkspacesLayout` is smaller than
adding deployment availability to `UserSystemContext` or introducing a new
layout context. TanStack Query deduplicates the same query key, and the hook's
module-level boot revision keeps the comparison page-scoped.

### Keep status metadata in the header

The revision and age are useful even while the accordion is collapsed. Keeping
`DeployStatus` in `headerExtra` also avoids duplicating the established compact
format/link/accessibility behavior.

### Use a section action for Refresh

A section action is reachable in collapsed and expanded states and already
isolates action activation from disclosure toggling. Adding explicit labeling
to the generic action contract improves accessibility without inventing a
Deploy-Status-specific header component.

### Use intrinsic section sizing

Deploy Status has status/actions rather than a scrollable content body. It must
remain `flex-none h-auto` so an expanded empty body cannot take drawer height
from Issue/Git/other content sections.

## Alternatives rejected

- Moving update detection into `DeployStatus`: rejected because `packages/ui`
  must remain presentational and must not fetch application state.
- Keeping Refresh in both places: rejected because the task explicitly makes
  Deploy Status the owner and removes it below the headshot.
- Adding a new backend field or endpoint: rejected because update availability
  is already correctly derived from the polled running revision.
