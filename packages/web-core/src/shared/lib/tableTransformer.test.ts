import { describe, expect, it } from 'vitest';
import {
  $convertFromMarkdownString,
  $convertToMarkdownString,
  TRANSFORMERS,
} from '@lexical/markdown';
import {
  $isTableCellNode,
  $isTableNode,
  TableCellHeaderStates,
  TableCellNode,
  TableNode,
  TableRowNode,
} from '@lexical/table';
import { HeadingNode, QuoteNode } from '@lexical/rich-text';
import { ListItemNode, ListNode } from '@lexical/list';
import { CodeHighlightNode, CodeNode } from '@lexical/code';
import { AutoLinkNode, LinkNode } from '@lexical/link';
import { $getRoot, createEditor, type LexicalNode } from 'lexical';
import { TABLE_TRANSFORMER } from '@vibe/ui/lib/table-transformer';

const transformers = [TABLE_TRANSFORMER, ...TRANSFORMERS];

type Block =
  | { type: 'table'; rows: { header: boolean; cells: string[] }[] }
  | { type: string; text: string };

function describeBlock(node: LexicalNode): Block {
  if (!$isTableNode(node)) {
    return { type: node.getType(), text: node.getTextContent() };
  }
  return {
    type: 'table',
    rows: node.getChildren().map((row) => {
      const cells = (row as TableRowNode).getChildren();
      return {
        header: cells.some(
          (cell) =>
            $isTableCellNode(cell) &&
            cell.getHeaderStyles() !== TableCellHeaderStates.NO_STATUS
        ),
        cells: cells.map((cell) => cell.getTextContent()),
      };
    }),
  };
}

function render(markdown: string): { blocks: Block[]; exported: string } {
  const editor = createEditor({
    nodes: [
      TableNode,
      TableRowNode,
      TableCellNode,
      HeadingNode,
      QuoteNode,
      ListNode,
      ListItemNode,
      CodeNode,
      CodeHighlightNode,
      LinkNode,
      AutoLinkNode,
    ],
    onError: (error) => {
      throw error;
    },
  });
  editor.update(() => $convertFromMarkdownString(markdown, transformers), {
    discrete: true,
  });
  return editor.getEditorState().read(() => ({
    blocks: $getRoot().getChildren().map(describeBlock),
    exported: $convertToMarkdownString(transformers),
  }));
}

function onlyTable(markdown: string) {
  const { blocks } = render(markdown);
  const tables = blocks.filter((b) => b.type === 'table');
  expect(tables).toHaveLength(1);
  return (tables[0] as Extract<Block, { type: 'table' }>).rows;
}

describe('TABLE_TRANSFORMER', () => {
  it('keeps an empty top-left header cell and drops the divider', () => {
    const rows = onlyTable(
      [
        'Intro.',
        '',
        '| | **22: store 187 Park Meadows** | **23: store 055 River North** |',
        '|---|---|---|',
        '| On the network? | Yes. Port 28. | Yes. Port 8. |',
        '| Windows name | **DESKTOP-5G7PLC7** | USSG01PC0100055 |',
        '',
        'After.',
      ].join('\n')
    );

    expect(rows).toEqual([
      {
        header: true,
        cells: ['', '22: store 187 Park Meadows', '23: store 055 River North'],
      },
      {
        header: false,
        cells: ['On the network?', 'Yes. Port 28.', 'Yes. Port 8.'],
      },
      {
        header: false,
        cells: ['Windows name', 'DESKTOP-5G7PLC7', 'USSG01PC0100055'],
      },
    ]);
  });

  it.each([
    '|---|---|',
    '| --- | --- |',
    '|:---|---:|',
    '| :---: | :-: |',
    '|  ---  |  ---  |',
    '| --- | --- |   ',
  ])('treats %j as a divider that marks the header', (divider) => {
    const rows = onlyTable(`| A | B |\n${divider}\n| 1 | 2 |`);

    expect(rows).toEqual([
      { header: true, cells: ['A', 'B'] },
      { header: false, cells: ['1', '2'] },
    ]);
  });

  it('drops a divider that is not under the first row', () => {
    const rows = onlyTable('| A | B |\n|---|---|\n| 1 | 2 |\n|---|---|');

    expect(rows).toEqual([
      { header: true, cells: ['A', 'B'] },
      { header: false, cells: ['1', '2'] },
    ]);
  });

  it('leaves a divider with no table above it as text', () => {
    const { blocks } = render('Before\n\n|---|---|');

    expect(blocks.some((b) => b.type === 'table')).toBe(false);
    expect(blocks).toContainEqual({ type: 'paragraph', text: '|---|---|' });
  });

  it('keeps empty leading, middle and trailing cells in place', () => {
    const rows = onlyTable(
      '| A | B | C |\n|---|---|---|\n| | 2 | 3 |\n| 1 | | 3 |\n| 1 | 2 | |\n| | | |'
    );

    expect(rows.map((r) => r.cells)).toEqual([
      ['A', 'B', 'C'],
      ['', '2', '3'],
      ['1', '', '3'],
      ['1', '2', ''],
      ['', '', ''],
    ]);
  });

  it('pads short rows and keeps extra cells', () => {
    const rows = onlyTable(
      '| A | B | C |\n|---|---|---|\n| 1 |\n| 1 | 2 | 3 | 4 |'
    );

    expect(rows.map((r) => r.cells)).toEqual([
      ['A', 'B', 'C'],
      ['1', '', ''],
      ['1', '2', '3', '4'],
    ]);
  });

  it('keeps an escaped pipe inside its cell', () => {
    const rows = onlyTable('| a \\| b | c |\n|---|---|\n| `x \\| y` | z |');

    expect(rows.map((r) => r.cells)).toEqual([
      ['a | b', 'c'],
      ['x | y', 'z'],
    ]);
  });

  it('splits on a pipe after an escaped backslash', () => {
    const rows = onlyTable('| C:\\\\| next |\n|---|---|\n| a\\\\\\| b | c |');

    expect(rows.map((r) => r.cells)).toEqual([
      ['C:\\', 'next'],
      ['a\\| b', 'c'],
    ]);
  });

  it('renders a plain table as before, with its header marked', () => {
    const rows = onlyTable('| Name | Value |\n| --- | --- |\n| x | 1 |');

    expect(rows).toEqual([
      { header: true, cells: ['Name', 'Value'] },
      { header: false, cells: ['x', '1'] },
    ]);
  });

  it.each([
    '| | **A** | B |\n|---|---|---|\n| row | | 3 |',
    '| a \\| b | c |\n|---|---|\n| 1 | 2 |',
    '| C:\\\\| a\\\\\\| b |\n|---|---|\n| 1 | 2 |',
    '| C:\\\\new | two\\nlines |\n|---|---|\n| 1 | 2 |',
    '| A | B |\n|---|---|\n| 1 |',
  ])('re-imports its own export to the same table: %j', (markdown) => {
    const first = render(markdown);
    const second = render(first.exported);

    expect(second.blocks).toEqual(first.blocks);
    expect(render(second.exported).exported).toBe(second.exported);
  });

  it('saves a table without a divider as GFM with a header row', () => {
    const first = render('| A | B |\n| 1 | 2 |');
    expect(first.exported).toBe('| A | B |\n| --- | --- |\n| 1 | 2 |');

    const second = render(first.exported);
    expect(second.blocks).toEqual([
      {
        type: 'table',
        rows: [
          { header: true, cells: ['A', 'B'] },
          { header: false, cells: ['1', '2'] },
        ],
      },
    ]);
    expect(second.exported).toBe(first.exported);
  });
});
