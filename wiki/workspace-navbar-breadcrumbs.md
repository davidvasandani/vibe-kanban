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

There is a second, subtler race after initial loading: the user-workspaces and
project-issues shapes can both be ready at different database positions. A
workspace relationship can therefore name an issue that a ready, cached issue
shape has not observed yet. Before declaring that issue unavailable, query the
authoritative issue-detail endpoint by UUID. Treat the detail request as
loading while it is in flight, use its `simple_id` when it succeeds, and settle
to the unavailable state only for a confirmed miss or an exhausted request
error. This keeps independent shape cursors from producing a false unavailable
breadcrumb without leaving the navbar hidden forever on request failure.

The project level has the same cross-shape race. `useAllOrganizationProjects`
aggregates one Electric project collection per organization, while the selected
workspace relationship arrives through a separate source. A completed aggregate
that does not yet contain `workspace.project_id` is therefore not proof that the
project relationship disappeared. Keep the aggregate as the fast path, then use
the authenticated project-detail endpoint by UUID as the authoritative fallback.

Model project resolution explicitly as `loading`, `resolved`, or `unavailable`,
parallel to issue resolution. While project detail is loading, defer the linked
trail. A resolved project uses its human-readable name and UUID-backed navigation.
A settled miss or exhausted request error renders a non-actionable
`Project unavailable` crumb instead of collapsing to only the workspace title or
displaying the project UUID. Project and issue resolution can run concurrently;
the builder defers the whole hierarchy while either required identity is loading.

## Contributed by

- 6c5c-bread-crumbs-sho
- vk/719f-vk-workspace-iss
- vk/f195-bread-crumbs-are
