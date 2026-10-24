# Tasks: Collapse matching tool-call messages

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes.

## Phase 1: Setup
- [x] T001 Add `'tool'` to `ToolAggregationType` and `toolName?: string` to `AggregatedPatchGroup` in `packages/web-core/src/shared/hooks/useConversationHistory/types.ts`

## Phase 2: Core
- [x] T002 Group consecutive same-name `tool` entries, excluding `pending_approval`, set `toolName` on the group, and export `formatToolArgsPreview`, all in `packages/web-core/src/shared/lib/aggregateEntries.ts` (depends on T001; FR-1, FR-4, FR-5, FR-6)
- [x] T003 [P] Render `tool` groups in `AggregatedGroupEntry`: label = tool name, unit `call`, per-call argument preview and `getToolOutput` content, in `packages/web-core/src/features/workspace-chat/ui/DisplayConversationEntry.tsx` (depends on T002; FR-2, FR-3)

## Phase 3: Validation
- [x] T004 [P] Add unit tests for the grouping rules and `formatToolArgsPreview` in `packages/web-core/src/shared/lib/aggregateEntries.test.ts` (depends on T002)
- [x] T005 Run the web-core Vitest suite, `pnpm run check`, `pnpm run lint` and `pnpm run format` (repo-wide; depends on T003, T004)
