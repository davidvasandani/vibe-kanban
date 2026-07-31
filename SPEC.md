# Technical Spec: Concatenate Repeating Conversation Lines

## Summary

Vibe Kanban conversation logs can contain long uninterrupted runs of
`codex review --uncommitted` command rows. Rendering every occurrence separately
wastes vertical space, is especially disruptive on mobile, and obscures
meaningful progress.

Collapse each uninterrupted run into one visible conversation entry and annotate
that entry with a compact repeat indicator. Preserve entry ordering, interruption
boundaries, status semantics, and the ability for later streaming updates to
replace the correct visible entry.

## Scope

- Vibe Kanban service repository only.
- Conversation-log normalization in the executor layer.
- Repeated, semantically equivalent `codex review --uncommitted` command rows
  that are adjacent in the normalized conversation.
- Bounded repeat indicators so arbitrarily long runs do not cause unbounded
  output growth.
- Unit tests covering eligible runs, interruption boundaries, failures,
  approvals, overlapping calls, streaming updates, and large repeat counts.

## Out of Scope

- Changes to the shared homelab deployment repository or any other service.
- Deduplicating non-adjacent entries.
- Combining entries with different commands, content, status, output, or
  semantic type.
- Hiding failed executions or changing persisted raw execution logs.
- Frontend-only deduplication that would leave the normalized log stream noisy.

## Functional Requirements

1. When two or more eligible, semantically identical entries occur without an
   intervening visible entry, expose a single normalized entry.
2. The collapsed entry must communicate how many later successful repetitions
   occurred. Small counts may use check marks; large counts must use a bounded
   aggregate such as `✓ ×N`.
3. A different visible entry terminates the active run. A later identical entry
   begins a new run.
4. In-progress updates for the latest occurrence must continue replacing the
   collapsed row rather than inserting a duplicate.
5. A failed occurrence must remain visibly failed and must not be counted as a
   successful repetition.
6. Existing normalized patch indices must remain correct for append/replace
   consumers on desktop and mobile.

## Non-Functional Requirements

- Repeat tracking uses constant auxiliary state per tracked entry kind.
- Display size remains bounded independently of the number of repeats.
- Existing log formats and public API types remain compatible.
- Tests are deterministic and do not require external services.

## Acceptance Criteria

- A long adjacent run like the screenshots renders as one row with a compact
  repetition indicator.
- Mixed or interrupted sequences retain their original visible ordering.
- The most recent failed repeat remains a failed row and is not represented as a
  success tick.
- Counts beyond the inline threshold use a constant-size marker.
- Relevant executor unit tests, formatting, and repository checks pass.

## Risks and Mitigations

- **Incorrect patch target:** Track the normalized entry index and latest raw
  tool-call identifier together, and test streaming updates.
- **Over-aggressive equality:** Compare the semantic payload required by each
  supported entry kind; never collapse merely because rendered labels happen to
  match.
- **Protocol reordering:** Preserve latest-call ownership while allowing stale
  calls to finish their own distinct rows.
- **Unbounded marker strings:** Switch from inline ticks to an aggregate marker
  at a fixed threshold.
