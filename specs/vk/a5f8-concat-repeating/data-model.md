# Data Model: Breadcrumb Resolution State

No persisted model changes.

## Project state

- `loading`: linked project relationship exists; identity resolution is active.
- `resolved`: human-readable project name plus project navigation action.
- `unavailable`: resolution settled without a displayable project; render a
  fixed non-actionable label.

## Issue state

Existing states remain:

- `none`: no linked issue relationship.
- `loading`: linked issue identity resolution is active.
- `resolved`: issue `simple_id` plus issue navigation action.
- `unavailable`: fixed non-actionable issue label.

The builder returns no trail while either required entity is loading. Otherwise
it emits project, optional issue, and workspace entries in hierarchy order.
