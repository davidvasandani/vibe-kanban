import { useEffect } from 'react';
import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext';
import { $isLinkNode, LinkNode } from '@lexical/link';
import { $getNearestNodeFromDOMNode, $getNodeByKey } from 'lexical';

// Only the canonical UUID route is allowed; arbitrary relative paths (including
// protocol-relative URLs) remain disabled. Both web apps own this route.
// Hex digits are case-insensitive, the path segments are not: the app serves
// only the lowercase route, so `/PROJECTS/...` would be a dead link.
const uuid = '[0-9a-fA-F]{8}(?:-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}';
const issueRoute = new RegExp(`^/projects/${uuid}/issues/${uuid}$`);

function updateLink(dom: HTMLAnchorElement, href: string) {
  const trimmed = href.trim();
  const clickable = /^https:\/\//i.test(trimmed) || issueRoute.test(trimmed);

  if (clickable) {
    dom.setAttribute('href', trimmed);
    dom.setAttribute('target', '_blank');
    dom.setAttribute('rel', 'noopener noreferrer');
    dom.style.removeProperty('cursor');
    dom.style.removeProperty('pointer-events');
    dom.removeAttribute('role');
    dom.removeAttribute('aria-disabled');
    dom.removeAttribute('title');
    dom.onclick = (e) => e.stopPropagation();
  } else {
    dom.removeAttribute('href');
    dom.removeAttribute('target');
    dom.removeAttribute('rel');
    dom.style.cursor = 'not-allowed';
    dom.style.pointerEvents = 'none';
    dom.setAttribute('role', 'link');
    dom.setAttribute('aria-disabled', 'true');
    // Keep the destination discoverable on hover even though it is inert.
    dom.title = trimmed;
    dom.onclick = null;
  }
}

/** Keep external HTTPS and explicit issue links usable in read-only messages. */
export function ReadOnlyLinkPlugin() {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    const unregister = editor.registerMutationListener(
      LinkNode,
      (mutations) => {
        editor.read(() => {
          for (const [nodeKey, mutation] of mutations) {
            if (mutation === 'destroyed') continue;
            const dom = editor.getElementByKey(nodeKey);
            const node = $getNodeByKey(nodeKey);
            if (dom instanceof HTMLAnchorElement && $isLinkNode(node)) {
              // Read the model: a previous pass may have removed the DOM href.
              updateLink(dom, node.getURL());
            }
          }
        });
      }
    );

    editor.read(() => {
      editor
        .getRootElement()
        ?.querySelectorAll('a')
        .forEach((dom) => {
          const node = $getNearestNodeFromDOMNode(dom);
          if ($isLinkNode(node)) updateLink(dom, node.getURL());
        });
    });

    return unregister;
  }, [editor]);

  return null;
}
