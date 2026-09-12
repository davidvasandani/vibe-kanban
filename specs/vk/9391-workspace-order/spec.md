# Feature Specification: Stable Workspace Order During Restart

**Feature dir**: `specs/vk/9391-workspace-order/`
**Status**: Draft

## Summary

Keep the workspace sidebar meaningfully ordered from its first post-restart
render, even though richer activity summaries arrive after the base workspace
records. Users should not have to wait for every workspace to be reread before
recent work appears in the expected position.

## User Stories

- As a Vibe Kanban user with many workspaces, I want the workspace list to be
  usefully ordered immediately after a restart so I can resume work without
  waiting for background summary loading.
- As a user who chose a workspace sort direction, I want that choice to remain
  effective during partial loading and after richer activity data arrives.
- As a user who pins workspaces, I want pinned workspaces to remain ahead of
  unpinned workspaces throughout restart recovery.

## Functional Requirements

- FR-1: The workspace list must produce a meaningful updated-time order as soon
  as persisted workspace records are available.
- FR-2: When a richer latest-activity time exists for a workspace, the updated
  sort must prefer it over the base workspace update time.
- FR-3: When richer activity data is absent or invalid, the updated sort must
  fall back to the persisted workspace update time.
- FR-4: A workspace with no valid selected timestamp must not precede a
  workspace with a valid selected timestamp solely because its data is missing.
- FR-5: Pinned workspaces must precede unpinned workspaces regardless of sort
  field or direction.
- FR-6: The selected ascending or descending direction must apply within each
  pinning group.
- FR-7: Equal or missing timestamps must resolve deterministically so identical
  inputs always produce identical ordering.
- FR-8: The same ordering contract must apply to active and archived workspace
  lists.
- FR-9: Existing search, project filtering, pull-request filtering, pagination,
  and user sort-preference behavior must remain intact.

## Out of Scope

- Changing the workspace summary API or its refresh interval.
- Persisting a separate user-defined manual workspace order.
- Changing workspace grouping, filtering, archive semantics, or pin controls.
- Changing any service other than Vibe Kanban or its deployment configuration.

## Acceptance Criteria

- [ ] With base workspace records present and no summaries loaded, updated-time
      sorting uses persisted workspace update times rather than workspace names.
- [ ] Once summaries are present, their latest activity times take precedence
      over base update times.
- [ ] Missing or malformed timestamps sort behind valid timestamps in both
      ascending and descending modes.
- [ ] Pinned workspaces remain first and timestamp ties resolve consistently.
- [ ] Active and archived workspace collections use the same ordering policy.
- [ ] Automated tests cover the base-only and enriched startup states.
- [ ] Vibe Kanban's required formatting and proportionate frontend verification
      pass.

## Open Questions

None. See `clarifications.md` for the resolved decisions.
