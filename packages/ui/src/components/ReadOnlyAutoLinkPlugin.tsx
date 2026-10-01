import { useEffect } from 'react';
import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext';
import {
  $createAutoLinkNode,
  $isAutoLinkNode,
  $isLinkNode,
  AutoLinkNode,
} from '@lexical/link';
import { $isCodeNode } from '@lexical/code';
import {
  $getNodeByKey,
  $getRoot,
  $isTextNode,
  HISTORY_MERGE_TAG,
  type LexicalNode,
  type TextNode,
} from 'lexical';

/** Updates that only add or remove read-only auto links; never exported. */
export const AUTO_LINK_UPDATE_TAG = 'vk-read-only-auto-link';

export interface UrlMatch {
  start: number;
  end: number;
  url: string;
}

// `<` and `>` end a URL so `<https://…>` links only the address.
const URL_PATTERN = /https?:\/\/[^\s<>]+/gi;
// Sentence punctuation that may follow a URL but is almost never part of it.
const TRAILING_PUNCTUATION = '.,;:!?\'"*`';
const CLOSERS: Record<string, string> = { ')': '(', ']': '[', '}': '{' };

function count(text: string, char: string) {
  return text.split(char).length - 1;
}

function trimUrl(url: string) {
  for (;;) {
    const last = url[url.length - 1];
    const opener = CLOSERS[last];
    if (
      TRAILING_PUNCTUATION.includes(last) ||
      // Keep balanced closers: `https://en.wikipedia.org/wiki/A_(b)`.
      (opener && count(url, last) > count(url, opener))
    ) {
      url = url.slice(0, -1);
    } else {
      return url;
    }
  }
}

/** Find bare http(s) URLs in plain text, minus trailing punctuation. */
export function findUrlMatches(text: string): UrlMatch[] {
  const matches: UrlMatch[] = [];
  for (const match of text.matchAll(URL_PATTERN)) {
    const start = match.index;
    // `foohttps://…` is not a URL boundary.
    if (start > 0 && /[A-Za-z0-9]/.test(text[start - 1])) continue;
    const url = trimUrl(match[0]);
    if (/^https?:\/\/$/i.test(url)) continue;
    matches.push({ start, end: start + url.length, url });
  }
  return matches;
}

function $isInsideLinkOrCode(node: LexicalNode) {
  for (let parent = node.getParent(); parent; parent = parent.getParent()) {
    if ($isLinkNode(parent) || $isCodeNode(parent)) return true;
  }
  return false;
}

function $linkUrls(node: TextNode) {
  let current: TextNode | undefined = node;
  while (current) {
    // Code-block text is `CodeHighlightNode`, so this also excludes it; inline
    // code is simple text with a `code` format and is linked on purpose.
    if (!current.isSimpleText() || $isInsideLinkOrCode(current)) return;
    const [match] = findUrlMatches(current.getTextContent());
    if (!match) return;

    let urlNode: TextNode;
    if (match.start === 0) {
      [urlNode, current] = current.splitText(match.end);
    } else {
      [, urlNode, current] = current.splitText(match.start, match.end);
    }
    const link = $createAutoLinkNode(match.url);
    urlNode.replace(link);
    link.append(urlNode);
  }
}

function $getAutoLinks() {
  const links = new Set<AutoLinkNode>();
  for (const text of $getRoot().getAllTextNodes()) {
    const parent = text.getParent();
    if ($isAutoLinkNode(parent)) links.add(parent);
  }
  return links;
}

/** Whether the tree holds display-only links, so must not be exported. */
export function $hasAutoLinks() {
  return $getAutoLinks().size > 0;
}

function $unlinkAll() {
  for (const link of $getAutoLinks()) {
    for (const child of link.getChildren()) link.insertBefore(child);
    // Lexical's normalization merges the freed text back into its neighbours.
    link.remove();
  }
}

/**
 * Turn bare URLs into links while the editor is read-only.
 *
 * Linking splits text nodes, and markdown export of split formatted text is
 * not byte-identical (`**a https://b**` gains a `&#32;`). Read-only editors
 * still sync markdown out: issue descriptions wire `onChange`, and composers
 * are read-only only while sending. So links are added in a separate update
 * tagged `AUTO_LINK_UPDATE_TAG`, which `MarkdownSyncPlugin` does not export,
 * and removed again when the plugin unmounts (the editor becomes editable).
 * `ReadOnlyLinkPlugin` decides clickability.
 */
export function ReadOnlyAutoLinkPlugin() {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    if (!editor.hasNodes([AutoLinkNode])) {
      throw new Error(
        'ReadOnlyAutoLinkPlugin: AutoLinkNode not registered on editor'
      );
    }
    const update = (fn: () => void) =>
      editor.update(fn, { tag: [AUTO_LINK_UPDATE_TAG, HISTORY_MERGE_TAG] });
    // Lexical batches updates queued before the next commit and merges their
    // tags. Mount and unmount coincide with value changes (the initial parse,
    // a composer clearing after send), so queue behind that commit; otherwise
    // the tag would hide a real content change from `onChange`.
    const deferredUpdate = (fn: () => void) => queueMicrotask(() => update(fn));

    deferredUpdate(() => $getRoot().getAllTextNodes().forEach($linkUrls));
    const unregister = editor.registerUpdateListener(
      ({ dirtyLeaves, tags }) => {
        if (tags.has(AUTO_LINK_UPDATE_TAG) || dirtyLeaves.size === 0) return;
        update(() => {
          for (const key of dirtyLeaves) {
            const node = $getNodeByKey(key);
            if ($isTextNode(node) && node.isAttached()) $linkUrls(node);
          }
        });
      }
    );
    return () => {
      unregister();
      deferredUpdate($unlinkAll);
    };
  }, [editor]);

  return null;
}
