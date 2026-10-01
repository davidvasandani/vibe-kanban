# Prior knowledge: URLs always clickable

Task: `vk/e4ef-urls-always-clic`.

## Knowledge base search

I searched `wiki/` (see `wiki/INDEX.md`) for `lexical`, `markdown`,
`wysiwyg`, `autolink`, `linkify`, `link`, and `issue route`/`issue link`.
None of the pages cover markdown rendering or link handling. The only
`markdown` hit, `task-pipeline-block.md`, is about pipeline blocks in issue
descriptions and is unrelated. There is no prior wiki knowledge for this
area.

## Relevant history outside the wiki

- PR #313 (`b4e850b3`) rewrote `packages/ui/src/components/ReadOnlyLinkPlugin.tsx`:
  - It allow-lists external `https://` links and the canonical in-app issue
    route `/projects/<uuid>/issues/<uuid>`, matched case-sensitively.
  - Issue routes open in the same window, because Tauri's `on_new_window`
    would send `_blank` relative URLs to the system browser.
  - Disabled links get `pointer-events: none`, `role=link` and
    `aria-disabled`.
  - It runs from a `LinkNode` mutation listener, and it reads the URL from
    the model rather than the DOM, because an earlier pass may have removed
    the DOM `href`.
  - Tests are in `packages/web-core/src/shared/components/ReadOnlyLinkPlugin.test.tsx`
    (jsdom and a real `LexicalComposer`).
- The https-only rule comes from upstream (BloopAI). This task widens it to
  `http://` on purpose, because the user asked for every URL to be clickable.
  `javascript:` and `data:` stay blocked, because only the `http`/`https`
  schemes are allow-listed.
- `SimpleMarkdown.tsx` and `RawLogText.tsx` already linkify bare URLs with
  their own regexes. The Lexical read-only path is the one that does not.
