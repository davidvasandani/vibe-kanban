# Research: Desktop Deploy Status

## Decision 1: Reuse deployment metadata and semantics

The mobile feature already established the authoritative deployment identity:
the embedded running revision plus the immutable release build/publish timestamp
served by `/api/info`. Desktop consumes the same `useUserSystem` values. Process
start time, release-file reads, and a second endpoint are rejected because they
would create competing meanings or packaging dependencies.

## Decision 2: Compose at the workspace drawer boundary

`RightSidebar.tsx` is explicitly documented as the workspace right drawer and
is mounted behind the existing desktop right-sidebar visibility preference.
Adding the row there makes it first content whenever that drawer exists while
leaving route-specific project/issue panels alone.

## Decision 3: Fixed intrinsic row, not a section

The requirement says no toggle and always visible. The drawer's existing
`CollapsibleSectionHeader` intentionally owns disclosure state and persistence,
so using it would contradict the requirement or introduce a special-case fake
section. A plain `flex-none` row is both semantically honest and consistent with
the drawer's bounded-flex knowledge.

## Decision 4: Reuse shared presentation

`DeployStatus` already owns every behavioral edge case and its accessible label.
If desktop needs the age to ignore the mobile-only narrow-width hiding rule, an
optional component prop is preferable to duplicated markup or descendant CSS.
The compact mobile behavior remains the default to avoid regression.

## Dependencies

No new package or crate dependency is required.
