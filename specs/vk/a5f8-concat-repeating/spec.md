# Feature Specification: Concatenate Repeating Lines

**Feature dir**: `specs/vk/a5f8-concat-repeating/`
**Status**: Draft

## Summary

Compact uninterrupted runs of the same Codex review command in Vibe Kanban's
conversation history. Repeated review passes currently consume most of the
visible transcript—particularly on mobile—making meaningful reasoning and
results difficult to find. One row with a compact repeat marker should retain
the progress signal without the visual flood.

## User Stories

- As a user monitoring a long-running task, I want consecutive review passes
  summarized in one line so that meaningful progress remains visible.
- As a mobile user, I want repeated tool rows to consume minimal height so that
  I can navigate the conversation without excessive scrolling.
- As a user diagnosing a failed review pass, I want failures and interruptions
  to remain distinct so that compaction does not misrepresent execution state.

## Functional Requirements

- FR-1: The system must represent an uninterrupted run of identical
  `codex review --uncommitted` commands as one visible conversation row. Shell
  wrapping or an absolute path to the `codex` executable does not change
  eligibility after the command is normalized for display.
- FR-2: The visible row must indicate later successful repetitions with a
  compact marker.
- FR-3: Repeat markers must remain bounded in size for arbitrarily long runs.
- FR-4: The first occurrence must retain its existing visible representation.
- FR-5: A changed command or any intervening visible entry must end the current
  run.
- FR-6: A failed, denied, timed-out, or otherwise unsuccessful occurrence must
  remain visibly unsuccessful and must end successful-repeat reuse.
- FR-7: Updates belonging to an older occurrence must not overwrite the newest
  occurrence represented by a shared row.
- FR-8: Streaming updates for one occurrence must update that occurrence rather
  than incrementing the repeat count.
- FR-9: The behavior must be consistent for all supported Codex event formats.
- FR-10: Commands outside the eligible review-command scope must retain their
  existing one-row-per-execution behavior.

## Out of Scope

- Non-adjacent deduplication.
- Frontend-only hiding of existing rows.
- Compaction of arbitrary shell commands.
- Persisted raw-log rewriting.
- Changes to deployment configuration or any service outside Vibe Kanban.

## Acceptance Criteria

- [x] Three adjacent successful `codex review --uncommitted` executions produce
      one visible command row with two repetition ticks.
- [x] Ten total adjacent successful occurrences use a counted marker rather
      than nine individually allocated ticks.
- [x] An intervening assistant, thinking, tool, system, or error entry causes a
      later identical command to appear on a new row.
- [x] A changed review command appears on a new row.
- [x] Two identical non-review commands remain two rows.
- [x] A failed repeat remains visibly failed without a success tick and a later
      matching command starts a new run.
- [x] Repeated updates for the same command ID neither create another row nor
      increment the count.
- [x] Direct app-server and legacy Codex protocol fixtures exhibit equivalent
      behavior.

## Open Questions

None.
