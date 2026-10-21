# Fix markdown table rendering

Task: `vk/37ef-fix-tables`

## Problem and evidence

A conversation message containing a GFM table with an empty top-left header
cell rendered broken (screenshot attached to the task):

```markdown
| | **22: store 187 Park Meadows** | **23: store 055 River North** |
|---|---|---|
| On the network? | Yes. Wired on switch port 28 … | Yes. Wired on switch port 8 … |
```

Rendered result:

1. The header row has two cells instead of three. The store names shift one
   column left, so they sit over the wrong columns.
2. The `|---|---|---|` divider row is rendered as a body row of literal
   `---` cells.

Reproduced headlessly against `TABLE_TRANSFORMER` + Lexical 0.36.2:

```
TABLE
  ROW [ '[0]22: store 187 Park Meadows', '[0]23: store 055 River North' ]
  ROW [ '[0]---', '[0]---', '[0]---' ]
  ROW [ 'On the network?', … ]
```

No row is marked as a header (`[0]` = `NO_STATUS`) for *any* table, not only
this one.

## Root cause

Both defects are in `packages/ui/src/lib/table-transformer.ts`, the custom
Lexical element transformer used by `WYSIWYGEditor` (every read-only and
editable markdown surface: conversation entries, issue descriptions,
comments, notes).

1. **Divider never detected.** `replace` checks
   `parentNode.getTextContent()` against the divider regex. In Lexical 0.36
   `$importBlocks` calls `textNode.setTextContent(lineText.slice(match[0].length))`
   *before* calling `replace`, so for a full-line match the paragraph text is
   `""`. The divider branch is dead code, the line falls through to the row
   branch and becomes a `---` row, and no header row is ever marked.
2. **Empty cells dropped.** `mapToTableCells` does
   `.filter((c) => c.length > 0)` after splitting on `|`, so an empty cell
   anywhere in a row disappears and every later cell shifts left.

## Requirements

- R1. A GFM divider row (`|---|`, `| --- |`, `|:---|`, `| :---: |`,
  `|  ---  |`, with or without trailing whitespace) is never rendered as a
  row. When it directly follows the first row, that row becomes a header row.
- R2. Empty cells are preserved at their position (leading, middle,
  trailing), so columns line up with the header.
- R3. A body row shorter than the header is padded with empty cells so the
  grid is rectangular. Longer rows keep all their cells (no data loss).
- R4. Escaped pipes (`\|`) inside a cell stay in that cell as a literal `|`
  and export back as `\|`, so import/export round-trips.
- R5. Export stays GFM: header row, divider row, body rows; an empty cell
  exports as an empty cell, and re-importing the export gives the same
  table.
- R6. Existing tables without empty cells render as before, plus the header
  row is now correctly marked as a header.

## Non-goals

- Column alignment (`:---:`) styling. Alignment markers are accepted and
  discarded as before.
- Changing `MarkdownPreview` (react-markdown + remark-gfm), which already
  renders GFM tables correctly.
- Table editing UX (adding/removing rows) in the composer.

## Acceptance

- Unit tests in `packages/web-core` cover R1–R5 by importing markdown into a
  headless Lexical editor with `TABLE_TRANSFORMER` and asserting the table
  structure, header state and exported markdown.
- `pnpm run check`, `pnpm run lint`, and the web-core vitest suite pass.
