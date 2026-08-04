# Research: Restore Linked Workspace Breadcrumbs

## Existing failure boundary

`useAllOrganizationProjects` aggregates per-organization Electric collections.
Its completed array can temporarily omit a project named by a workspace record
because those independent synchronized sources need not share a cursor. The
navbar currently gates `buildWorkspaceBreadcrumbs` on a non-null collection
project, so this miss collapses the entire breadcrumb.

## Decision: authoritative project detail fallback

Use the existing authenticated `GET /v1/projects/{project_id}` route after the
collection is ready and misses. This is directly analogous to the shipped issue
detail fallback and confirms both membership access and current entity identity.

Alternatives rejected:

- Display the UUID: violates the human-readable identity contract.
- Keep waiting on the collection indefinitely: a stale/deleted relationship can
  leave the header hidden forever.
- Add a backend breadcrumb endpoint: unnecessary duplication because both
  project and issue detail contracts already exist.
- Change Electric synchronization: disproportionate to this presentation race.

## Decision: explicit unavailable project state

Render `Project unavailable` as a non-actionable first crumb after a settled
miss/error. This distinguishes a stale relationship from an unlinked workspace
and preserves hierarchy without asserting an unknown name.

## Dependencies

No new dependency. Use existing TanStack Query, remote API request/error helpers,
and Vitest infrastructure.
