# Markdown tables in the Lexical editor

How GFM tables are imported and exported by `TABLE_TRANSFORMER`
(`packages/ui/src/lib/table-transformer.ts`). It is first in
`WYSIWYGEditor`'s transformer list, so every markdown surface (conversation
entries, issue descriptions, comments, notes) uses it.

## The importer has already stripped the line

`@lexical/markdown` 0.36's `$importBlocks` calls
`textNode.setTextContent(lineText.slice(match[0].length))` **before**
`replace(elementNode, [textNode], match, true)`. For a full-line regex the
paragraph is empty by the time `replace` runs.

- Decide everything from `match`, never from
  `parentNode.getTextContent()`. Reading the paragraph is how the divider
  check silently became dead code: every divider rendered as a row of
  `---`, and no header row was ever marked.
- Returning `false` from `replace` does **not** restore the text. On import
  the line would vanish, so `$keepAsText` puts `match[0]` back when
  `isImport` is true. The typing-shortcut path (`runElementTransformers`)
  removes the matched text only when `replace` succeeds, so it must not be
  restored there.
- Any custom element transformer with a whole-line regex has the same trap.

## Cell parsing rules

- Keep empty cells. Filtering them out shifts every later cell one column
  left (a comparison table with an empty top-left cell put each heading
  over the wrong column).
- Split with GFM backslash parity, not a `(?<!\\)\|` lookbehind: `\|` is a
  literal pipe, `\\|` is an escaped backslash followed by a separator.
  `\n` (odd parity) is the exported line break and is decoded in the same
  pass. Other escapes are left in the cell for Lexical, which already maps
  `\\` ↔ `\` and `\*` ↔ `*`, but not `\|`.
- Only a divider directly under the first row marks a header. Later
  dividers are dropped. A divider with no table above it stays text.
- Short rows are padded to the first row's width. Extra cells are kept.

## Export

- Escape every `|` in cell markdown as `\|`. Lexical has already doubled
  literal backslashes, so parity survives the round trip.
- `traverseChildren(cell)` concatenates the cell's paragraphs with nothing
  between them. Export each block and join with `\n` (escaped to `\n`).
- A table with more than one row always exports a divider, so a table
  written without one is saved as GFM with a header row. This is a one-time
  normalisation and is stable afterwards.
- Before this fix, every save of a table added one more `| --- |` row,
  because the divider was re-imported as a row and exported again.

## Testing

`packages/web-core/src/shared/lib/tableTransformer.test.ts` imports with a
plain `createEditor` (no `@lexical/headless` dependency needed) and
`[TABLE_TRANSFORMER, ...TRANSFORMERS]`. It asserts rows, cell text and
header state, then checks that import → export → import is stable.

## Contributed by

- vk/37ef-fix-tables
