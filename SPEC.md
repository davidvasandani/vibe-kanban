# Technical Specification: Reliable Workspace Issue Breadcrumbs

Task: `vk/719f-vk-workspace-iss`

## Problem

The workspace navbar sometimes renders `Issue unavailable` in the breadcrumb
even though the workspace is linked to an issue. The issue relationship comes
from the selected remote workspace, while the display label comes from an
asynchronously synchronized project-issues collection. A temporarily empty
collection must not be treated as proof that the linked issue is unavailable.

## Desired behavior

- A workspace linked to an issue renders `Project / ISSUE-N / Workspace` once
  the issue record is available.
- During initial project-issue loading, the linked breadcrumb trail is deferred
  instead of showing either `Issue unavailable`, a raw issue UUID, or the
  structurally incomplete `Project / Workspace`.
- `Issue unavailable` is shown only after the issue query has completed and no
  displayable issue record exists.
- The resolved issue breadcrumb navigates to the linked project issue using
  entity UUIDs.
- An unavailable issue breadcrumb is non-interactive.
- Workspaces with no linked issue keep the existing
  `Project / Workspace` breadcrumb.

## Technical approach

Keep asynchronous entity resolution and navigation inside
`packages/web-core`; do not move data access into the presentational navbar in
`packages/ui`.

Model issue breadcrumb resolution explicitly:

1. `none`: no linked issue; render the existing two-level breadcrumb.
2. `loading`: a linked issue exists and its project-issue collection is still
   loading; return no custom breadcrumb until resolution settles.
3. `resolved`: render the issue `simple_id` between project and workspace and
   attach issue navigation.
4. `unavailable`: loading finished without a usable `simple_id`; preserve the
   hierarchy position with a non-link `Issue unavailable` label.

Use the `isLoading` signal returned by the project issue `useShape` query.
Do not infer loading from the collection's current length, since an empty
collection is also a valid settled result. Isolate breadcrumb construction in
a pure helper so timing-sensitive states can be covered without mounting the
full provider graph.

## Files expected to change

- `packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx`
- `packages/web-core/src/shared/components/ui-new/containers/navbarBreadcrumbs.ts`
- `packages/web-core/src/shared/components/ui-new/containers/navbarBreadcrumbs.test.ts`

Pipeline artifacts under `specs/` and the repository root will also be updated.

## Verification

- Unit tests cover linked loading, linked resolved, linked unavailable, and
  unlinked states.
- Tests assert breadcrumb order and labels, navigation behavior, absence of
  raw UUIDs, and absence of a false partial linked hierarchy while loading.
- Run the focused Vitest test, frontend typecheck, formatting, and the
  repository-required checks appropriate to the affected package.

## Non-goals

- Changing Electric collection synchronization or recovery behavior.
- Changing workspace-to-issue persistence.
- Editing generated shared types.
- Redesigning navbar presentation or truncation.
