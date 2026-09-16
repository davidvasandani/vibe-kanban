# Implementation Plan: Timestamp the Workspace Chat Log

**Spec**: `./spec.md`
**Status**: Ready

## Technical Context

The affected UI is React/TypeScript in `packages/web-core`, shared by local and
remote frontends. Conversation data uses generated `NormalizedEntry` types,
stable `PatchTypeWithKey` identities, aggregation, and TanStack virtualization.
Vitest covers pure and rendered behavior. No backend, schema, generated type,
dependency, or homelab change is needed.

## Architecture & Approach

1. In `deriveConversationEntries.ts`, copy each semantic execution process's
   `created_at` into synthetic user-message and script normalized entries.
2. Add a focused timestamp helper/component beside workspace-chat UI that:
   validates timestamps, selects an aggregate's latest valid member, formats
   compact/full local labels, and renders semantic metadata.
3. In `DisplayConversationEntry.tsx`, render that timestamp in the existing
   spaced row wrapper for meaningful log row families. Do not modify the row
   model, patch key, aggregation, ordering, or child component APIs.
4. Add pure helper tests and derivation tests. Add a rendered regression test
   for semantic `<time>` output and safe omission.

## Data Model

See `./data-model.md`. Existing data is sufficient.

## Contracts

See `./contracts.md`. Only a presentation contract changes.

## Research Notes

See `./research.md`. No new dependency is introduced.

## Constitution Check

- I/III/VI: one shared formatter and existing row boundary; no parallel model.
- II: pure formatting, derivation, and rendered-DOM contracts are testable.
- IV: feature data remains in `web-core`; no duplicate local/remote UI.
- XIV: use locked repository scripts after dependency preflight.
- XX: event/process timestamps remain authoritative; missing time stays absent;
  compact labels expose full local metadata.

No constitution deviations are required.

## Risks & Dependencies

- Locale/timezone-dependent tests can become flaky. Tests will inject a fixed
  `now` and formatter locale/options at the pure helper boundary.
- An outer metadata line adds row height. Existing ResizeObserver measurement
  handles actual height, and the row estimate already budgets message chrome;
  rendered structure and browser verification will check presentation.
- Some executor entries legitimately lack timestamps. Safe omission is an
  explicit contract, not a reason to fabricate time.
