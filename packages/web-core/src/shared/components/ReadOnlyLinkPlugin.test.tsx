/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { $convertFromMarkdownString, TRANSFORMERS } from '@lexical/markdown';
import { LexicalComposer } from '@lexical/react/LexicalComposer';
import { RichTextPlugin } from '@lexical/react/LexicalRichTextPlugin';
import { ContentEditable } from '@lexical/react/LexicalContentEditable';
import { LexicalErrorBoundary } from '@lexical/react/LexicalErrorBoundary';
import { $createLinkNode, $isLinkNode, LinkNode } from '@lexical/link';
import {
  $createParagraphNode,
  $createTextNode,
  $getRoot,
  type LexicalEditor,
} from 'lexical';
import { ReadOnlyLinkPlugin } from '@vibe/ui/components/ReadOnlyLinkPlugin';

const issueUrl =
  '/projects/11111111-1111-1111-1111-111111111111/issues/22222222-2222-2222-2222-222222222222';
// Letter-bearing UUIDs, so case handling is actually exercised.
const hexIssueUrl =
  '/projects/abcdef01-2345-6789-abcd-ef0123456789/issues/fedcba98-7654-3210-fedc-ba9876543210';
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

async function renderLink(href: string, plugin = true, markdown = false) {
  await act(async () => {
    root.render(
      <LexicalComposer
        initialConfig={{
          namespace: 'issue-links',
          editable: false,
          nodes: [LinkNode],
          onError: (error) => {
            throw error;
          },
          editorState: (instance) => {
            editor = instance;
            if (markdown) {
              $convertFromMarkdownString(
                `Created **[VAS-646](${href})**.`,
                TRANSFORMERS
              );
              return;
            }
            $getRoot().append(
              $createParagraphNode().append(
                $createLinkNode(href).append($createTextNode('ISSUE-1'))
              )
            );
          },
        }}
      >
        <RichTextPlugin
          contentEditable={<ContentEditable />}
          placeholder={null}
          ErrorBoundary={LexicalErrorBoundary}
        />
        {plugin && <ReadOnlyLinkPlugin />}
      </LexicalComposer>
    );
  });
  return container.querySelector('a')!;
}

async function changeHref(href: string) {
  await act(async () => {
    editor.update(() => {
      const node = $getRoot().getFirstDescendant()?.getParent();
      if (!$isLinkNode(node)) throw new Error('Missing link node');
      node.setURL(href);
    });
  });
  return container.querySelector('a')!;
}

describe('read-only issue references', () => {
  it('renders an agent Markdown issue reference as a usable link', async () => {
    const link = await renderLink(issueUrl, true, true);
    expect(link.textContent).toBe('VAS-646');
    expect(link.getAttribute('href')).toBe(issueUrl);
  });

  it('preserves an accessible issue anchor and stops parent click handling', async () => {
    const link = await renderLink(issueUrl);
    expect(link.getAttribute('href')).toBe(issueUrl);
    expect(link.getAttribute('target')).toBe('_blank');
    expect(link.getAttribute('rel')).toBe('noopener noreferrer');
    expect(link.getAttribute('aria-disabled')).toBeNull();
    expect(link.tabIndex).toBe(0);
    let bubbled = false;
    container.addEventListener('click', () => {
      bubbled = true;
    });
    link.dispatchEvent(
      new MouseEvent('click', { bubbles: true, cancelable: true })
    );
    expect(bubbled).toBe(false);
  });

  it('handles links already present when the plugin mounts', async () => {
    await renderLink(issueUrl, false);
    const link = await renderLink(issueUrl);
    expect(link.getAttribute('href')).toBe(issueUrl);
  });

  it.each([
    'https://example.com/issue/1',
    hexIssueUrl,
    hexIssueUrl
      .toUpperCase()
      .replace('/PROJECTS/', '/projects/')
      .replace('/ISSUES/', '/issues/'),
  ])('preserves allowed destination %s', async (href) => {
    expect((await renderLink(href)).getAttribute('href')).toBe(href);
  });

  it.each([
    'javascript:alert(1)',
    'data:text/html,test',
    '//evil.example/x',
    '/settings',
    '/projects/p/issues/i',
    `${issueUrl}/../settings`,
    `${issueUrl}?redirect=evil`,
    '#fragment',
    '../file',
    'http://example.com',
    // The app only serves the lowercase route, so a shouted path is a dead link.
    issueUrl.replace('/projects/', '/Projects/'),
    issueUrl.replace('/issues/', '/Issues/'),
  ])('disables unsupported destination %s', async (href) => {
    const link = await renderLink(href);
    expect(link.hasAttribute('href')).toBe(false);
    expect(link.getAttribute('aria-disabled')).toBe('true');
    expect(link.title).toBe(href);
  });

  it('restores interactivity after a disabled link changes to an issue', async () => {
    await renderLink('/settings');
    const link = await changeHref(issueUrl);
    expect(link.getAttribute('href')).toBe(issueUrl);
    expect(link.style.pointerEvents).toBe('');
    expect(link.getAttribute('aria-disabled')).toBeNull();
    expect(link.hasAttribute('title')).toBe(false);
    const disabled = await changeHref('javascript:alert(1)');
    expect(disabled.hasAttribute('href')).toBe(false);
    expect(disabled.hasAttribute('target')).toBe(false);
  });
});
