# Tasks: MCP Tool Count and Last-Checked Time

**Feature**: `specs/vk/dcf7-vk-mcp-add-tool/`  
**Task**: `dcf7-vk-mcp-add-tool`

## Layer 1 — Pure presentation contract

- [x] T001 Add a pure MCP tool-count aggregation and locale-aware checked-time
  formatting helper in `packages/web-core/src/shared/lib/mcpCheckSummary.ts`.
- [x] T002 Add focused Vitest coverage in
  `packages/web-core/src/shared/lib/mcpCheckSummary.test.ts` for absent results,
  failed/missing counts, singular/equal counts, divergent ranges, and stable
  timestamp formatting. Depends on T001.
- [x] T003 [P] Add matching MCP check-summary translation keys to all seven
  `packages/web-core/src/i18n/locales/*/settings.json` files.

## Layer 2 — Settings integration

- [x] T004 Add `checkedAtByServer` state to `McpSettingsSection.tsx`; stamp only
  unique returned logical servers with a single post-response completion time.
  Depends on T001.
- [x] T005 Clear timestamp state wherever configuration reload/save clears MCP
  test results, while preserving unaffected timestamps on targeted retests.
  Depends on T004.
- [x] T006 Render the count/range and localized checked time on each server card
  using responsive existing design tokens. Depends on T001, T003, T004.

## Layer 3 — Verification

- [x] T007 Run the focused helper tests and relevant existing MCP frontend tests.
  Depends on T002, T006.
- [x] T008 Run the web-core TypeScript check. Depends on T006.
- [x] T009 Run repository formatting, rerun affected validation, and inspect the
  diff for unrelated/generated changes. Depends on T007, T008.

## Layer 4 — Review and knowledge

- [x] T010 Run an independent Codex review of the diff, fix confirmed significant
  findings, rerun relevant validation, and repeat review until clean. Depends on
  T009.
- [x] T011 Update `docs/knowledge-base/mcp-connectivity-testing.md` and
  `docs/knowledge-base/INDEX.md` with reusable count aggregation/timestamp
  lifecycle guidance tagged `dcf7-vk-mcp-add-tool`, then commit the knowledge
  base update. Depends on T010.

## Parallel execution notes

- T003 is independent of helper implementation and may run alongside T001/T002.
- T007 and T008 are separate read-only checks and may run in parallel after the
  integration is complete.
- Implementation edits touching `McpSettingsSection.tsx` remain sequential to
  avoid overlapping state/render modifications.
