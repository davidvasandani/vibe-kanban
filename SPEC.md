# Technical Specification: MCP Tool Count and Last-Checked Time

## Summary

Enhance the shared MCP settings view so each configured server card reports the
number of tools discovered by its most recent successful connectivity check and
the time that check completed. The presentation should follow the supplied
Ohana reference while fitting Vibe Kanban's existing compact settings cards.

## Current behavior

- The shared MCP settings screen can test all configured assignments or one
  server at a time.
- The backend probe already performs `tools/list` and returns `tool_count` in
  each `McpServerTestResult`.
- Results are held in component state and used to render per-assignment status
  icons and failure/authentication details.
- No completion timestamp is recorded, and successful result metadata is not
  shown on the server card.

## Required behavior

1. After a server test completes, its card shows a concise metadata line with
   the discovered tool count and a localized last-checked time.
2. Tool counts come from successful `McpServerTestResult.tool_count` values;
   unavailable counts must not be represented as zero.
3. A server assigned to multiple executors must have a deterministic aggregate
   display. Identical successful counts display once. If successful assignments
   report different counts, the UI must communicate the range rather than pick
   an arbitrary assignment.
4. The last-checked time represents when the latest test response for that
   server was received in the current UI session.
5. Testing all servers updates each returned server independently; testing one
   server updates only that server's metadata.
6. Retesting preserves prior metadata until the replacement response arrives,
   then atomically replaces the affected server's displayed count/time.
7. Loading a fresh configuration, saving/reloading, or completing an OAuth flow
   must avoid associating stale results with changed server definitions.
8. The metadata remains readable on narrow layouts and uses the existing design
   system and translation infrastructure.

## Data and API impact

No backend API or persistence change is required. `McpServerTestResult` already
contains `tool_count`; the checked timestamp is client-observed ephemeral UI
state. No generated shared types should be edited for this feature.

## Accessibility and localization

- Use normal text, not icon-only communication, for count and checked time.
- Add translatable strings for singular/plural tool counts, count ranges, and
  the checked-time phrase in every locale or use language-neutral interpolation
  patterns consistent with this repository's localization policy.
- Render dates with the user's locale via `Intl`/existing date helpers.

## Verification

- Unit-test aggregation and timestamp formatting/state behavior, including no
  result, one count, identical counts, differing counts, and missing counts.
- Exercise the MCP settings component/type checks to ensure existing test,
  OAuth, dirty-state, and responsive behavior remains intact.
- Run repository formatting and focused frontend checks/tests.

## Non-goals

- Persisting health history or timestamps across page reloads.
- Periodic/background probing.
- Changing the MCP probe protocol or backend timeout behavior.
- Redesigning the MCP settings screen beyond the requested metadata.
