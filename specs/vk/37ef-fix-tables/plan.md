# Implementation Plan: Markdown tables render as written

**Spec**: `specs/vk/37ef-fix-tables/spec.md`
**Branch**: `vk/37ef-fix-tables`

## Technical context

- Frontend only. One source file changes:
  `packages/ui/src/lib/table-transformer.ts` (`TABLE_TRANSFORMER`, a Lexical
  `ElementTransformer`).
- Consumer: `packages/web-core/src/shared/components/WYSIWYGEditor.tsx`
  puts `TABLE_TRANSFORMER` first in `allTransformers`; every markdown
  surface in local-web and remote-web goes through it (IV: both frontends
  are the blast radius; no API change for either).
- Lexical `^0.36.2` (`@lexical/markdown`, `@lexical/table`). No new
  dependency.
- Tests: vitest in `packages/web-core` (`vitest.config.ts`, node
  environment), which imports `@vibe/ui/lib/table-transformer`.

## Root cause (see research.md)

1. `replace` reads `parentNode.getTextContent()`, which
   `@lexical/markdown` 0.36 `$importBlocks` has already reduced to the text
   after the match (empty for a full-line match). The divider branch is
   unreachable.
2. `mapToTableCells` filters out empty strings after `split('|')`.

## Design

### Import (`replace`)

1. `const line = match[0]`. If `TABLE_ROW_DIVIDER_REG_EXP.test(line)`:
   - previous sibling is a table → if it has exactly one row, mark its cells
     `TableCellHeaderStates.ROW`; remove the paragraph; done (FR-001/002).
   - no previous table → `return false` so the line stays ordinary text
     (no table invented from a stray divider).
2. Otherwise `mapToTableCells(match[1])`:
   - split on unescaped pipes `/(?<!\\)\|/`, trim, keep empty strings;
   - replace `\|` with `|` in each cell; convert `\n` escapes as before;
   - parse each cell's markdown into a `TableCellNode` as before.
3. If the previous sibling is a table, pad the new row with empty cells up
   to the first row's cell count (never truncate) and append; else create a
   table (FR-003/004).

Divider regex becomes `/^\|(?:\s*:?-+:?\s*\|)+\s*$/` (FR-001 variants).
Row regex `^(?:\|)(.+)(?:\|)\s?$` is kept; it already captures the
content between the outer pipes.

### Export

Unchanged structure. Each cell's markdown has `|` escaped as `\|` (skipping
already-escaped pipes) so cells containing a pipe re-import as one cell
(FR-005/006). Empty cells already export as `|  |`, which the new splitter
keeps.

## Constitution check

| Principle | How it's honoured |
| --- | --- |
| I Clarity | Small, local changes in one transformer; comments explain the Lexical import contract. |
| II Test the contract | vitest covers AC-1…AC-6 through the real transformer list. |
| III Small steps | No new plumbing; one file plus a test. |
| IV Shared boundaries | Change lives in `@vibe/ui`; both frontends benefit, no consumer API change. |
| XLVII / LVIII | Parses from `match`; export re-imports to the same structure; tests assert the node tree. |
| XLVIII Lint | No directive comments; test is a `.ts` file. |

## Files

- `packages/ui/src/lib/table-transformer.ts` — fix.
- `packages/web-core/src/shared/lib/tableTransformer.test.ts` — new tests.

## Verification

`pnpm --filter @vibe/web-core test`, `pnpm run check`, `pnpm run lint`,
`pnpm run format`.
