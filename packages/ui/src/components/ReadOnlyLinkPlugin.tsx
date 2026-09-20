import { useEffect } from 'react';
import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext';
import { $isLinkNode, LinkNode } from '@lexical/link';
import { $getNodeByKey } from 'lexical';

// Only the canonical UUID route is allowed; arbitrary relative paths (including
// protocol-relative URLs) remain disabled. Both web apps own this route.
// Matched case-sensitively throughout: issue lookup is an exact match against
// the lowercase UUIDs Postgres emits, and route params are never normalised, so
// `/PROJECTS/...` or an upper-case UUID would only ever be a dead link.
const uuid = '[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}';
const issueRoute = new RegExp(`^/projects/${uuid}/issues/${uuid}$`);

function updateLink(dom: HTMLAnchorElement, href: string) {
  const trimmed = href.trim();
  const external = /^https:\/\//i.test(trimmed);
  const clickable = external || issueRoute.test(trimmed);

  if (clickable) {
    dom.setAttribute('href', trimmed);
    if (external) {
      dom.setAttribute('target', '_blank');
      dom.setAttribute('rel', 'noopener noreferrer');
    } else {
      // Issue routes belong to this app: keep them in the current window.
      // `_blank` is wrong for them because in the Tauri build `on_new_window`
      // denies the window and hands the relative URL to the system browser,
      // dropping the user out of the desktop app. (Either way this is a full
      // document navigation, not a client-side route change — see the
      // knowledge base for why that is accepted for now.)
      dom.removeAttribute('target');
      dom.removeAttribute('rel');
    }
    dom.style.removeProperty('pointer-events');
    dom.removeAttribute('role');
    dom.removeAttribute('aria-disabled');
    dom.onclick = (e) => e.stopPropagation();
  } else {
    dom.removeAttribute('href');
    dom.removeAttribute('target');
    dom.removeAttribute('rel');
    dom.style.pointerEvents = 'none';
    dom.setAttribute('role', 'link');
    dom.setAttribute('aria-disabled', 'true');
    // No `title` or `cursor` hint here: `pointer-events: none` stops the anchor
    // being hit-tested, so neither could ever render. Giving disabled links real
    // feedback means dropping `pointer-events` and guarding the click instead,
    // which changes click-through behaviour — out of scope here.
    dom.onclick = null;
  }
}

/** Keep external HTTPS and explicit issue links usable in read-only messages. */
export function ReadOnlyLinkPlugin() {
  const [editor] = useLexicalComposerContext();

  useEffect(
    () =>
      // `skipInitialization` defaults to false, so links that already exist when
      // this mounts arrive here as 'created' — no separate initial sweep needed.
      editor.registerMutationListener(LinkNode, (mutations) => {
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
      }),
    [editor]
  );

  return null;
}
