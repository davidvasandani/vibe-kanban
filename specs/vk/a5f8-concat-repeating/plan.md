# Implementation Plan: Concatenate Repeating Lines

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context

The change is confined to Rust in
`crates/executors/src/executors/codex/normalize_logs.rs`. That module consumes
both `codex_app_server_protocol` item notifications and legacy
`codex_protocol::EventMsg` events, and writes normalized JSON patches through a
shared `EntryIndexProvider` into `MsgStore`.

No storage migration, public API/type change, frontend change, deployment
change, or new dependency is required.

## Architecture & Approach

### Shared repeat state

Extend `LogState` with one optional `RepeatedCommand` value. It records the
original normalized entry index and display command, number of total
occurrences, latest call ID, whether that latest occurrence completed
successfully, and the last successful normalized-entry snapshot.

### Eligibility and display

Add a predicate for the normalized `codex review --uncommitted` operation. It
accepts the existing shell-unwrapped display form, including an absolute path to
the `codex` executable, but rejects other arguments and arbitrary repeated
commands.

Add `repeat_ticks(total_count)` with the established threshold: up to eight
repetitions render inline ticks; larger runs render `✓ ×N`. `CommandState`
carries its current repeat count, while rendering excludes the in-flight
occurrence until it succeeds.

### Lifecycle

Centralize command start and completion behavior in `LogState` helpers used by
both protocol branches:

1. On an eligible new call ID, reuse the tracked index only when the command is
   identical, the latest occurrence succeeded, and the shared entry index shows
   no intervening allocation.
2. Store every in-flight call in `commands` for ID-based routing, while marking
   the newest call ID as owner of the shared row.
3. Streaming updates for that same ID replace the shared row without changing
   the repeat count.
4. Completion updates repeat ownership only for its latest call ID; older calls
   still complete their distinct rows. Success arms the next repeat. Failure
   restores the prior successful aggregate and moves the failed call to a new
   row before disarming reuse.
5. Non-eligible commands keep the existing fresh-index path.

The current Codex normalizer does not reset its `EntryIndexProvider` during a
session, so no additional reset hook is required.

## Data Model

See `./data-model.md`.

## Contracts

See `./contracts/normalized-patch-stream.md`.

## Research Notes

See `./research.md`.

## Constitution Check

- Principle II: focused fixtures cover adjacency, status, ownership, marker
  bounds, and both protocol formats.
- Principles III and VI: reuse the existing server-normalizer compaction
  pattern and make the smallest command-specific extension.
- Principle IX: equality, adjacency, completion, latest-owner, and bounded-marker
  invariants are explicit.
- Constraints: no generated files, dependency, remote mutation, destructive
  operation, or external service are involved; `pnpm run format` will run.

No constitution deviation is planned.

## Risks & Dependencies

- Different protocol formats may provide differently wrapped command strings.
  The predicate uses the same shell-unwrapped representation already exposed in
  `ActionType::CommandRun`, and fixtures cover both paths.
- Late events could overwrite a newer repeat. Latest-call ownership prevents
  stale replacement.
- A generic compactor could hide meaningful repeated shell work. Eligibility is
  deliberately limited to the reported review operation.
- Reusing one row necessarily exposes only the newest occurrence's detailed
  result. This matches existing Grok compaction and is limited to review passes
  whose visible flood is the reported defect.
