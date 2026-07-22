# Implementation Plan: MCP Tool Count and Last-Checked Time

1. Inspect the constitution and current shared MCP test/card contracts; confirm
   that the existing `tool_count` response is sufficient and that timestamps
   should remain frontend-only.
2. Define a pure MCP check-summary model/helper that:
   - selects successful results with known tool counts,
   - returns one count for equal results or a min/max range for divergent ones,
   - associates the summary with the per-server response completion timestamp,
   - exposes locale-aware checked-time formatting inputs.
3. Add focused unit tests for no results, failed/missing counts, a single count,
   identical multi-executor counts, divergent counts, and timestamp formatting.
4. Extend `McpSettingsSection` state with per-server checked timestamps. When a
   test response lands, update returned assignment results and stamp each
   returned logical server using one captured completion time.
5. Clear checked timestamps at the same configuration-invalidating boundaries
   as test results (initial load/reload and save refresh), while leaving
   unaffected servers intact after a targeted retest.
6. Render the aggregate tool count and localized checked time in each server
   card, matching existing compact responsive styles and preserving current
   assignment status/failure/OAuth rendering.
7. Add the required settings strings to all supported locales with matching key
   shapes and correct singular/plural/range interpolation.
8. Run focused unit tests, TypeScript checks, formatting, and relevant repository
   validation. Inspect the final diff for generated-file or unrelated changes.
9. Run an independent Codex review, fix confirmed significant findings, and
   repeat verification/review until clean.
10. Update the MCP connectivity-testing knowledge page and index with reusable
    aggregation/timestamp lifecycle guidance tagged `dcf7-vk-mcp-add-tool`, then
    commit the knowledge-base update as required by the pipeline.

## Expected file scope

- `packages/web-core/src/shared/lib/` — pure summary helper and tests.
- `packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx`
  — state ingestion, lifecycle, and card rendering.
- `packages/web-core/src/i18n/locales/*/settings.json` — user-facing strings.
- `specs/vk/dcf7-vk-mcp-add-tool/` — SpecKit artifacts.
- `docs/knowledge-base/` — reusable shipped knowledge and index refresh.

No backend, database, API contract, or generated shared-type changes are
expected.
