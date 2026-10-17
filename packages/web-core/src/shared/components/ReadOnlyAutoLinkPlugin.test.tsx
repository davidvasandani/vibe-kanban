/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { $convertToMarkdownString, TRANSFORMERS } from '@lexical/markdown';
import { LexicalComposer } from '@lexical/react/LexicalComposer';
import { RichTextPlugin } from '@lexical/react/LexicalRichTextPlugin';
import { ContentEditable } from '@lexical/react/LexicalContentEditable';
import { LexicalErrorBoundary } from '@lexical/react/LexicalErrorBoundary';
import { HeadingNode, QuoteNode } from '@lexical/rich-text';
import { ListItemNode, ListNode } from '@lexical/list';
import { CodeHighlightNode, CodeNode } from '@lexical/code';
import { $createAutoLinkNode, AutoLinkNode, LinkNode } from '@lexical/link';
import {
  $createTextNode,
  $getRoot,
  type LexicalEditor,
  type ParagraphNode,
} from 'lexical';
import {
  findUrlMatches,
  ReadOnlyAutoLinkPlugin,
} from '@vibe/ui/components/ReadOnlyAutoLinkPlugin';
import { ReadOnlyLinkPlugin } from '@vibe/ui/components/ReadOnlyLinkPlugin';
import { MarkdownSyncPlugin } from '@vibe/ui/components/MarkdownSyncPlugin';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;
let container: HTMLDivElement;
let root: Root;
let editor: LexicalEditor;

beforeEach(() => {
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
});
afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

const urls = (text: string) => findUrlMatches(text).map((m) => m.url);

describe('findUrlMatches', () => {
  it.each([
    ['see https://example.com.', ['https://example.com']],
    ['https://example.com/a, b', ['https://example.com/a']],
    ['done: https://x.y/pull/1803!', ['https://x.y/pull/1803']],
    ['(https://example.com)', ['https://example.com']],
    ['(see https://w.org/A_(b))', ['https://w.org/A_(b)']],
    ['"https://example.com/q?a=1&b=2"', ['https://example.com/q?a=1&b=2']],
    ["'https://example.com'.", ['https://example.com']],
    ['<https://example.com/x>', ['https://example.com/x']],
    ['[https://example.com]', ['https://example.com']],
    ['**https://example.com**', ['https://example.com']],
    ['http://localhost:3000/p#h', ['http://localhost:3000/p#h']],
    ['HTTPS://EXAMPLE.COM', ['HTTPS://EXAMPLE.COM']],
    ['a https://a.io and https://b.io', ['https://a.io', 'https://b.io']],
    ['https://', []],
    ['https://.', []],
    ['foohttps://example.com', []],
    ['ftp://example.com javascript:alert(1)', []],
  ])('%s', (text, expected) => {
    expect(urls(text)).toEqual(expected);
  });

  it('reports offsets into the source text', () => {
    expect(findUrlMatches('go (https://a.io).')).toEqual([
      { start: 4, end: 16, url: 'https://a.io' },
    ]);
  });
});

// Mirrors WYSIWYGEditor: markdown arrives through MarkdownSyncPlugin, and the
// auto-link plugin is mounted only while read-only.
function Editor({
  markdown,
  readOnly = true,
  onChange,
}: {
  markdown: string;
  readOnly?: boolean;
  onChange?: (markdown: string) => void;
}) {
  return (
    <LexicalComposer
      initialConfig={{
        namespace: 'auto-links',
        editable: false,
        nodes: [
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
        editorState: (instance) => {
          editor = instance;
        },
      }}
    >
      <MarkdownSyncPlugin
        value={markdown}
        onChange={onChange}
        editable={!readOnly}
        transformers={TRANSFORMERS}
      />
      <RichTextPlugin
        contentEditable={<ContentEditable />}
        placeholder={null}
        ErrorBoundary={LexicalErrorBoundary}
      />
      {readOnly && <ReadOnlyAutoLinkPlugin />}
      {readOnly && <ReadOnlyLinkPlugin />}
    </LexicalComposer>
  );
}

async function render(markdown: string) {
  await act(async () => {
    root.render(<Editor markdown={markdown} />);
  });
  return [...container.querySelectorAll('a')];
}

const exportMarkdown = () =>
  editor.getEditorState().read(() => $convertToMarkdownString(TRANSFORMERS));

describe('ReadOnlyAutoLinkPlugin', () => {
  it('links a bare PR URL at the end of a sentence', async () => {
    const url =
      'https://github.com/sweetgreen/terraform-infrastructure/pull/1803';
    const [link, ...rest] = await render(
      `It is pushed to PR #1803, which has a new description: ${url}. Nothing has been applied yet.`
    );
    expect(rest).toHaveLength(0);
    expect(link.getAttribute('href')).toBe(url);
    expect(link.textContent).toBe(url);
    expect(link.getAttribute('target')).toBe('_blank');
    expect(link.getAttribute('rel')).toBe('noopener noreferrer');
    expect(link.nextSibling?.textContent).toMatch(/^\. Nothing/);
  });

  it('links URLs in parentheses, lists, headings and bold text', async () => {
    const links = await render(
      [
        '# Docs https://h.io',
        '',
        '- item (https://a.io/x_(y)) and https://b.io',
        '',
        '**see https://c.io**',
      ].join('\n')
    );
    expect(links.map((a) => a.getAttribute('href'))).toEqual([
      'https://h.io',
      'https://a.io/x_(y)',
      'https://b.io',
      'https://c.io',
    ]);
    expect(links[3].querySelector('strong')).not.toBeNull();
  });

  it('makes plain http URLs clickable', async () => {
    const [bare, explicit] = await render(
      'dev http://localhost:3000 and [lan](http://172.16.0.150/)'
    );
    expect(bare.getAttribute('href')).toBe('http://localhost:3000');
    expect(explicit.getAttribute('href')).toBe('http://172.16.0.150/');
    expect(explicit.getAttribute('target')).toBe('_blank');
  });

  it('links inline code but not fenced code blocks', async () => {
    const links = await render(
      [
        'run `https://inline.io` then',
        '',
        '```',
        'https://fenced.io',
        '```',
      ].join('\n')
    );
    expect(links.map((a) => a.getAttribute('href'))).toEqual([
      'https://inline.io',
    ]);
    expect(links[0].querySelector('code')).not.toBeNull();
  });

  it('leaves explicit markdown links and unsafe schemes alone', async () => {
    const links = await render(
      '[docs](https://docs.io/https://other.io) and [x](javascript:alert)'
    );
    expect(links).toHaveLength(2);
    expect(links[0].getAttribute('href')).toBe(
      'https://docs.io/https://other.io'
    );
    expect(links[1].hasAttribute('href')).toBe(false);
    expect(links[1].getAttribute('aria-disabled')).toBe('true');
  });

  it('links URLs that arrive in a later value', async () => {
    expect(await render('loading')).toHaveLength(0);
    const links = await render('done: https://a.io/1 and https://b.io/2');
    expect(links.map((a) => a.getAttribute('href'))).toEqual([
      'https://a.io/1',
      'https://b.io/2',
    ]);
  });

  const roundTrip = [
    'See https://a.io/x_(y). Also **bold https://b.io** and `https://c.io`.',
    '',
    '- [named](https://d.io) http://localhost:3000',
  ].join('\n');

  it('never reports display links through onChange', async () => {
    const changes: string[] = [];
    await act(async () => {
      root.render(
        <Editor markdown={roundTrip} onChange={(m) => changes.push(m)} />
      );
    });
    expect(container.querySelectorAll('a')).toHaveLength(5);

    // Later untagged updates while the links are shown (a reader selecting
    // text, or a node refreshing) must not export the split tree either.
    await act(async () => {
      editor.update(() => {
        $getRoot().selectStart();
      });
    });
    await act(async () => {
      editor.update(() => {
        for (const node of $getRoot().getAllTextNodes()) node.markDirty();
      });
    });
    expect(container.querySelectorAll('a')).toHaveLength(5);

    // Baseline: the same value in an editor that never auto-links.
    const baseline: string[] = [];
    const other = document.createElement('div');
    const otherRoot = createRoot(other);
    await act(async () => {
      otherRoot.render(
        <LexicalComposer
          initialConfig={{
            namespace: 'baseline',
            editable: false,
            nodes: [ListNode, ListItemNode, CodeNode, LinkNode],
            onError: (error) => {
              throw error;
            },
          }}
        >
          <MarkdownSyncPlugin
            value={roundTrip}
            onChange={(m) => baseline.push(m)}
            editable={false}
            transformers={TRANSFORMERS}
          />
          <RichTextPlugin
            contentEditable={<ContentEditable />}
            placeholder={null}
            ErrorBoundary={LexicalErrorBoundary}
          />
        </LexicalComposer>
      );
    });
    act(() => otherRoot.unmount());
    expect(changes).toEqual(baseline);
  });

  it('keeps syncing an editable editor that holds a pasted auto link', async () => {
    const changes: string[] = [];
    await act(async () => {
      root.render(
        <Editor
          markdown="draft"
          readOnly={false}
          onChange={(m) => changes.push(m)}
        />
      );
    });
    // What Lexical's clipboard format yields when a rendered message is
    // pasted into a composer: an AutoLinkNode in an editable tree.
    await act(async () => {
      editor.update(() => {
        const paragraph = $getRoot().getFirstChildOrThrow<ParagraphNode>();
        paragraph.append(
          $createTextNode(' see '),
          $createAutoLinkNode('https://a.io').append(
            $createTextNode('https://a.io')
          )
        );
      });
    });
    expect(changes.at(-1)).toBe('draft see https://a.io');
  });

  it('removes display links when the editor becomes editable', async () => {
    await render(roundTrip);
    expect(container.querySelectorAll('a')).toHaveLength(5);
    await act(async () => {
      root.render(<Editor markdown={roundTrip} readOnly={false} />);
    });
    // Only the explicit markdown link is left, and the text is whole again.
    expect(container.querySelectorAll('a')).toHaveLength(1);
    expect(exportMarkdown()).toBe(
      'See https://a.io/x\\_(y). Also **bold https://b.io** and `https://c.io`.\n\n- [named](https://d.io) http://localhost:3000'
    );
  });
});
