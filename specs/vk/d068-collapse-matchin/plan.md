# Implementation Plan: Collapse matching tool-call messages

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context
This is a frontend-only change in TypeScript/React 18. The grouping logic lives
in `packages/web-core`, a shared library used by both local-web and remote-web.
The accordion UI is `ChatAggregatedToolEntries` in `packages/ui`, which needs
no change. Tests use Vitest. Rust, the database and generated types are
unaffected.

## Architecture & Approach
Generic and MCP tool calls arrive as `NORMALIZED_ENTRY` → `tool_use`, with
`action_type.action === 'tool'` and `tool_name` set to `mcp:<server>:<tool>`
for MCP tools (`crates/executors/src/executors/claude.rs`,
`codex/normalize_logs.rs`, `cursor.rs`).

1. `packages/web-core/src/shared/hooks/useConversationHistory/types.ts`:
   add `'tool'` to `ToolAggregationType`, and add an optional `toolName` to
   `AggregatedPatchGroup`.
2. `packages/web-core/src/shared/lib/aggregateEntries.ts`:
   - `getAggregationType` returns `'tool'` for `action === 'tool'` when the
     status is not `pending_approval` (FR-5).
   - The main loop also tracks `currentToolName`. A `tool` entry whose name
     differs from the current run flushes the run (FR-1, FR-4).
   - `flushToolGroup` attaches `toolName`. Single entries still pass through
     unchanged (FR-6).
   - Export a pure `formatToolArgsPreview(args: JsonValue | null): string`
     (FR-3).
3. `packages/web-core/src/features/workspace-chat/ui/DisplayConversationEntry.tsx`
   → `AggregatedGroupEntry`:
   - For `tool` entries, summary = `formatToolArgsPreview(arguments)` or
     the tool name, and content = `getToolOutput(entryType, content)`, which
     is the same output a single row opens.
   - `getDisplayProps` `case 'tool'` → `{ label: group.toolName, icon:
     ListMagnifyingGlassIcon, unit: 'call' }` (FR-2).
4. Group identity is unchanged (`agg:<first patchKey>`). The persisted
   expansion key `tool:${patchKey}` therefore stays stable while the run grows
   (FR-7). The timeline, row model and timestamp code already handle any
   `AGGREGATED_GROUP` generically, so no change is needed there (FR-8).

## Data Model
No persisted data. `AggregatedPatchGroup` gains `toolName?: string`, which
only exists on the client. See `research.md`.

## Contracts
No API change. The interface contract is the exported function
`formatToolArgsPreview`, which is specified in `research.md`.

## Research Notes
See `./research.md`.

## Constitution Check
- II Test the contract: `aggregateEntries.test.ts` covers FR-1, FR-4, FR-5 and
  FR-6, plus the preview helper.
- III Small, reversible steps: this extends the existing aggregation path and
  component, with no new grouping system.
- IV Shared-component boundaries: the `packages/ui` primitive is reused
  unchanged, and the data shaping stays in `web-core`. Both frontends are
  affected, and both render through the same `DisplayConversationEntry`.
- XLVIII Lint: `pnpm run lint` passes, with no directive comments.
No deviations.

## Risks & Dependencies
- Merging calls that need approval would hide the approval card. This is
  mitigated by FR-5 and a test.
- A run that is still streaming changes from a single row into a group when
  the second call arrives. The key is derived from the first entry, so the
  expansion state is not lost, but the row type changes once. Existing groups
  already behave this way.
