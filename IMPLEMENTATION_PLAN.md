# Implementation plan — vk/37ef-fix-tables

See `SPEC.md` for the problem, root cause and requirements R1–R6.

## Steps

1. **Detect the divider from the match, not the paragraph text**
   (`packages/ui/src/lib/table-transformer.ts`, `replace`).
   - Test `match[0]` against a loosened divider regex
     `^\|(?:\s*:?-+:?\s*\|)+\s*$`. `match[0]` is the full line in both the
     import path and the typing-shortcut path, unlike
     `parentNode.getTextContent()`, which Lexical 0.36 empties first.
   - When the previous sibling is a table with exactly one row, mark that
     row's cells `TableCellHeaderStates.ROW`. Always remove the divider
     paragraph.
2. **Preserve empty cells and escaped pipes** (`mapToTableCells`).
   - Split `match[1]` on unescaped pipes (`/(?<!\\)\|/`), trim each cell,
     keep empties, and turn `\|` back into `|` before parsing the cell
     content.
3. **Rectangular grid.** When appending a row to an existing table, pad it
   with empty cells up to the first row's cell count. Never truncate.
4. **Export.** Escape `|` in cell text as `\|` so an exported cell with a
   pipe re-imports as one cell. Empty cells already export as `|  |`.
5. **Tests** — `packages/web-core/src/shared/lib/tableTransformer.test.ts`:
   - the screenshot table (empty top-left header cell, bold headers);
   - divider variants (`|---|`, `| :---: |`, `|  ---  |`, trailing space);
   - empty leading/middle/trailing cells;
   - short body row padded;
   - escaped pipe kept in its cell;
   - header row marked;
   - import → export → import gives the same structure.
6. **Verify**: web-core vitest, `pnpm run check`, `pnpm run lint`,
   `pnpm run format`.
