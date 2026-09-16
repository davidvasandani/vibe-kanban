# Feature Specification: Timestamp the Workspace Chat Log

**Feature dir**: `specs/vk/a22f-time-stamp-chat/`
**Status**: Clarified

> The checked-in `/speckit.specify` prompt names an unrelated existing feature
> path. This task uses the current branch/task-owned path named above so prior
> feature artifacts are not overwritten.

## Summary

Show when each meaningful item in a workspace conversation occurred so users
can understand the timing and sequence of messages, agent events, and actions
without consulting raw logs.

## User Stories

- As a user reviewing a workspace conversation, I want every message to show
  when it was sent so that I can reconstruct the discussion chronology.
- As a user monitoring an active agent, I want events and actions to show when
  they occurred so that I can distinguish current progress from older output.
- As a user reviewing a past task, I want compact times to reveal an exact local
  date and time so that abbreviated labels are not ambiguous.

## Functional Requirements

- FR-1: Every visible logged message, agent event, and action displays a compact
  timestamp.
- FR-2: The displayed timestamp reflects when the underlying logged item
  occurred, not when the browser received, replayed, grouped, or rendered it.
- FR-3: A visible item derived from an execution record uses the execution's
  recorded creation time when it has no independent logged timestamp.
- FR-4: A visible row that groups multiple logged items displays one meaningful
  timestamp for that group.
- FR-5: The full localized date and time is available to assistive technology
  and as additional detail for the compact timestamp.
- FR-6: Missing or malformed timestamps do not cause a failure and do not get
  replaced with a fabricated current time.
- FR-7: Timestamp presentation does not change conversation ordering, item
  identity, grouping, paging, or navigation behavior.
- FR-8: The behavior is shared by local and remote workspace-chat consumers.

## Out of Scope

- Changing stored timestamp values, database schemas, or service APIs.
- Timestamping transient loading indicators, composer drafts, or navigation
  controls that are not logged conversation activity.
- Changing conversation retention, ordering, aggregation, or virtualization.
- Changes to another service or to homelab deployment configuration.

## Acceptance Criteria

- [ ] Representative user and assistant messages render a compact timestamp
      backed by the authoritative entry or execution record.
- [ ] Representative system events and tool actions render timestamps.
- [ ] Grouped activity renders one timestamp without requiring expansion.
- [ ] The timestamp exposes the full localized date and time.
- [ ] Missing and malformed values render safely without showing the current
      time or an invalid date label.
- [ ] Stable row keys, chronological order, and existing chat interactions are
      unchanged.
- [ ] Focused tests, frontend checks, lint, and formatting pass.
- [ ] Independent Codex review reports no significant findings.

## Open Questions

None. The clarification stage established a time-only label for entries on the
user's current local day, a short local date plus time for older/future entries,
and the most recent valid member timestamp for a grouped row.
