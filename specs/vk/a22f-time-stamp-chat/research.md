# Research: Timestamp the Workspace Chat Log

## Existing data path

- `NormalizedEntry` already has `timestamp: string | null`; generated shared
  types require no schema change.
- `useConversationHistory` materializes server normalized entries as
  `PatchTypeWithKey` objects and preserves their content. Most executor
  normalizers currently leave `NormalizedEntry.timestamp` empty, so keyed
  patches also need their server-recorded process creation time as a fallback.
- `deriveConversationEntries.ts` creates synthetic user-message and script rows
  but currently assigns `timestamp: null` even though each semantic process
  exposes `executionProcess.created_at`.
- `DisplayConversationEntry.tsx` is the common renderer for atomic and grouped
  rows across local and remote web clients. Its outer spaced wrapper has access
  to the complete `DisplayEntry`, making it the narrow shared presentation
  boundary for timestamp metadata.
- Aggregated rows retain their ordered member entries in `group.entries`; the
  most recent valid timestamp can be derived without altering aggregation.

## Decisions

### R1. Preserve timestamps during derivation

Synthetic rows inherit `executionProcess.created_at`, and loaded normalized rows
retain it as parent fallback metadata. This is authoritative and replay-safe,
unlike `new Date()` at render time.

### R2. Present metadata at the shared row boundary

Render one timestamp adjacent to the existing row from
`DisplayConversationEntrySpaced`. This covers all message/action/event
components without widening every `packages/ui` component API or duplicating
formatting. It also leaves semantic keys and component interaction untouched.

### R3. Use native locale formatting and semantic HTML

Use `Intl.DateTimeFormat`/`Date` and a `<time dateTime>` element. No dependency
is needed. The compact label is time-only for the local current day and includes
a short date otherwise; title and accessible text expose full local date/time.

### R4. Fail absent on invalid input

Parsing produces `null` for absent/non-finite dates. The renderer omits the
metadata instead of displaying `Invalid Date` or a fabricated current time.

### R5. Groups use their latest valid member time

A collapsed group communicates activity completed through its latest member.
When expanded, nested content remains part of that semantic row rather than
creating new virtualized rows, so the group-level label stays the relevant
timestamp.

## Alternatives rejected

- Adding timestamp props to every `@vibe/ui` chat component: large API churn and
  easy coverage gaps for specialized tool renderers.
- Stamping rows when received: dates replayed history incorrectly and violates
  authoritative event provenance.
- New standalone timestamp rows: changes row count, keys, navigation, and
  virtualized scroll geometry.
