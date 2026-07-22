# Clarifications: MCP Tool Count and Last-Checked Time

`/speckit.clarify` found no blocking open questions after comparing the request,
Ohana reference, existing API, and project knowledge.

## Resolved decisions

1. **When is “checked” recorded?** When the frontend receives a completed test
   response, not when the request starts and not on the backend host.
2. **Does a failed check receive a checked timestamp?** Yes. It was checked even
   if no tool count could be discovered; the existing status/diagnostic explains
   the outcome.
3. **What happens with multiple executor assignments?** Known counts from
   successful results are deduplicated; equal counts render once and unequal
   counts render as a min/max range.
4. **Is metadata persisted?** No. It is ephemeral state tied to the current
   loaded configuration and is cleared with stale test results.
5. **What date format is required?** A compact localized date-and-time string
   from the active i18n locale. Exact punctuation/order is locale-owned.
6. **Should stale metadata disappear while a retest is in flight?** No. Keep the
   prior result visible until the response arrives, then replace the affected
   server atomically.
7. **Does the backend need work?** No. It already returns `tool_count`; adding a
   server timestamp would misrepresent client receipt time and expand scope.
