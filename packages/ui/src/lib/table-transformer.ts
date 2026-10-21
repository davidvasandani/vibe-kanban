import {
  ElementTransformer,
  $convertFromMarkdownString,
  TRANSFORMERS,
} from '@lexical/markdown';
import {
  TableNode,
  TableRowNode,
  TableCellNode,
  $createTableNode,
  $createTableRowNode,
  $createTableCellNode,
  $isTableNode,
  $isTableRowNode,
  $isTableCellNode,
  TableCellHeaderStates,
} from '@lexical/table';
import { $isTextNode, type LexicalNode } from 'lexical';

const TABLE_ROW_REG_EXP = /^(?:\|)(.+)(?:\|)\s*$/;
const TABLE_ROW_DIVIDER_REG_EXP = /^\|(?:\s*:?-+:?\s*\|)+\s*$/;
// Cell separators are pipes not escaped with a backslash.
const CELL_SEPARATOR_REG_EXP = /(?<!\\)\|/;

function $createTableCell(textContent: string): TableCellNode {
  textContent = textContent.replace(/\\n/g, '\n').replace(/\\\|/g, '|');
  const cell = $createTableCellNode(TableCellHeaderStates.NO_STATUS);
  $convertFromMarkdownString(textContent, TRANSFORMERS, cell);
  return cell;
}

function splitTableCells(textContent: string): string[] {
  // Empty cells are kept so later cells stay in their column.
  return textContent.split(CELL_SEPARATOR_REG_EXP).map((c) => c.trim());
}

// Declines the match and leaves the line as ordinary text. On import the
// markdown importer has already stripped the matched text from the line, so
// it is put back; the typing path only removes it when replace succeeds.
function $keepAsText(
  children: Array<LexicalNode>,
  match: Array<string>,
  isImport: boolean
): false {
  const [first] = children;
  if (isImport && $isTextNode(first)) {
    first.setTextContent(match[0] + first.getTextContent());
  }
  return false;
}

export const TABLE_TRANSFORMER: ElementTransformer = {
  dependencies: [TableNode, TableRowNode, TableCellNode],
  type: 'element',
  regExp: TABLE_ROW_REG_EXP,

  export: (node, traverseChildren) => {
    if (!$isTableNode(node)) return null;

    const output: string[] = [];
    const children = node.getChildren();

    for (let i = 0; i < children.length; i++) {
      const row = children[i];
      if (!$isTableRowNode(row)) continue;

      const cells = row.getChildren();
      const cellTexts = cells.map((cell) => {
        if (!$isTableCellNode(cell)) return '';
        return traverseChildren(cell)
          .replace(/\|/g, '\\|')
          .replace(/\n/g, '\\n');
      });

      output.push('| ' + cellTexts.join(' | ') + ' |');

      // Add header divider after first row if it contains header cells
      if (i === 0) {
        const isHeader = cells.some(
          (cell) =>
            $isTableCellNode(cell) &&
            cell.getHeaderStyles() !== TableCellHeaderStates.NO_STATUS
        );
        if (isHeader || children.length > 1) {
          output.push('| ' + cellTexts.map(() => '---').join(' | ') + ' |');
        }
      }
    }

    return output.join('\n');
  },

  replace: (parentNode, children, match, isImport) => {
    // Decide from the match: the markdown importer strips the matched text
    // from parentNode before calling replace, so its text content is empty.
    const prevSibling = parentNode.getPreviousSibling();
    if (TABLE_ROW_DIVIDER_REG_EXP.test(match[0])) {
      // A divider without a table above it is ordinary text.
      if (!$isTableNode(prevSibling)) {
        return $keepAsText(children, match, isImport);
      }
      // Only a divider directly under the first row makes it a header.
      const firstRow = prevSibling.getFirstChild();
      if (prevSibling.getChildrenSize() === 1 && $isTableRowNode(firstRow)) {
        firstRow.getChildren().forEach((cell) => {
          if ($isTableCellNode(cell)) {
            cell.setHeaderStyles(TableCellHeaderStates.ROW);
          }
        });
      }
      parentNode.remove();
      return;
    }

    // Parse row cells. An all-empty row only counts inside a table.
    const cellTexts = splitTableCells(match[1]);
    if (!$isTableNode(prevSibling) && cellTexts.every((c) => c.length === 0)) {
      return $keepAsText(children, match, isImport);
    }
    const cells = cellTexts.map($createTableCell);

    const tableRow = $createTableRowNode();
    cells.forEach((cell) => tableRow.append(cell));

    // Check if previous sibling is a table to merge into
    if ($isTableNode(prevSibling)) {
      // Pad short rows to the first row's width; never drop extra cells.
      const firstRow = prevSibling.getFirstChild();
      const columnCount = $isTableRowNode(firstRow)
        ? firstRow.getChildrenSize()
        : 0;
      for (let i = cells.length; i < columnCount; i++) {
        tableRow.append($createTableCell(''));
      }
      prevSibling.append(tableRow);
      parentNode.remove();
    } else {
      // Create new table
      const table = $createTableNode();
      table.append(tableRow);
      parentNode.replace(table);
    }
  },
};
