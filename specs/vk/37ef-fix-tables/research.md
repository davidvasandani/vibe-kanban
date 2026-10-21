# Research: Markdown tables render as written

## R1. Why the divider is never detected

`@lexical/markdown` 0.36.2, `LexicalMarkdown.dev.js`, `$importBlocks`:

```js
const textNode = $createTextNode(lineText);
const elementNode = $createParagraphNode();
elementNode.append(textNode);
rootNode.append(elementNode);
for (const { regExp, replace } of elementTransformers) {
  const match = lineText.match(regExp);
  if (match) {
    textNode.setTextContent(lineText.slice(match[0].length));
    if (replace(elementNode, [textNode], match, true) !== false) break;
  }
}
```

The text node is truncated before `replace` runs, so for a full-line match
`parentNode.getTextContent()` is `""`. The typing-shortcut path
(`runElementTransformers`) also splits the matched text off. `match[0]` is
the matched line in both paths, so the decision must be made from it.

**Decision**: test the divider regex against `match[0]`.

## R2. Reproduction

Headless editor (`createEditor` with table nodes) and
`$convertFromMarkdownString(md, [TABLE_TRANSFORMER, ...TRANSFORMERS])` on
the reported table produced:

```
ROW [ '[0]22: store 187 Park Meadows', '[0]23: store 055 River North' ]
ROW [ '[0]---', '[0]---', '[0]---' ]
```

Both defects, and no header state on any row.

## R3. Empty cells

`split('|').map(trim).filter(len > 0)` drops empty cells. GFM keeps them.
**Decision**: keep empties; split on unescaped pipes only.

## R4. Ragged rows

GFM pads short rows and drops extra cells. **Decision**: pad, but do not
drop extra cells; losing agent-reported data is worse than a ragged edge.

## Alternatives rejected

- Switching read-only rendering to react-markdown (`MarkdownPreview`):
  much larger change, loses Lexical-only nodes (attachments, PR comments,
  read-only links, LVIII/XLVII behaviour).
- A table-level post-pass to fix headers after import: duplicates parsing
  and still depends on the lost text.
