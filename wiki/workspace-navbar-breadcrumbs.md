# Workspace navbar breadcrumbs and async issue identity

Workspace-page breadcrumbs are assembled in
`packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx`
and rendered by the presentational `packages/ui` `Navbar`. Keep entity lookup,
loading-state interpretation, and navigation callbacks in web-core; the shared
UI component should only receive prepared `NavbarBreadcrumbItem[]` data.

## The linked issue is relationship truth

A remote workspace's non-null `issue_id` proves that the workspace occupies an
issue position in the hierarchy, even when the project issue shape is initially
empty. Do not interpret an absent row during asynchronous collection startup as
an unlinked workspace. Doing so produces the misleading partial trail
`Project / Workspace`.

The relationship's `issue_id` is an internal UUID used for lookup and routing.
It is not the user-facing issue identifier. Only the resolved issue's
`simple_id` (for example `VK-123`) is suitable as the issue breadcrumb label.

## Model breadcrumb resolution explicitly

The workspace breadcrumb builder uses four issue states:

- `none`: the workspace has no linked issue; preserve `Project / Workspace`.
- `loading`: the relationship exists but issue data is still loading; defer the
  linked breadcrumb trail so a false partial hierarchy does not flash.
- `resolved`: render `Project / simple_id / Workspace`; bind navigation using
  the linked project and issue UUIDs, not the label.
- `unavailable`: loading completed without a displayable issue; retain the
  issue position as a non-link labeled `Issue unavailable`, never the UUID.

A small pure builder (`navbarBreadcrumbs.ts`) makes this state mapping testable
without mounting the container's provider graph or simulating Electric timing.
Tests should assert both positive labels/order and negative invariants: no raw
UUID, no partial linked hierarchy during loading, and no click action on an
unavailable issue.

## Related loading behavior

`useShape` reports the initial query state through `isLoading`; its returned
data is empty while disabled or loading. Keep those two facts separate in the
container. Collection fallback/recovery belongs to the Electric layer and does
not need to change for a breadcrumb state-classification fix.

## Contributed by

- 6c5c-bread-crumbs-sho
