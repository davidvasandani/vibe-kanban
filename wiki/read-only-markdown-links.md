# Read-only markdown links

How links behave in read-only markdown. That means every `WYSIWYGEditor`
rendered with `disabled`: conversation entries, issue descriptions, comments,
approvals and notes. It also covers why read-only Lexical changes must never
reach markdown sync.

## Two plugins, two jobs

- `ReadOnlyAutoLinkPlugin` (`packages/ui/src/components/`) finds bare
  `http(s)://` URLs and wraps them in `AutoLinkNode`s.
  - `findUrlMatches` does not require a word boundary before the URL, so
    `(https://…)`, `"https://…"` and `<https://…>` all link.
  - It trims trailing `.,;:!?'"*` and backticks, and removes `)]}` only when
    unbalanced, so Wikipedia's `A_(b)` keeps its bracket.
  - It skips text in links and `CodeNode`s. Inline code is linked on purpose.
- `ReadOnlyLinkPlugin` is the single clickability policy for every link
  class. It allows `http`/`https`, which open in a new tab with
  `noopener noreferrer`, and the canonical issue route, which opens in place.
  Everything else gets `pointer-events: none` and `aria-disabled`.
  - **Gotcha:** Lexical mutation listeners match the exact class.
    `AutoLinkNode` extends `LinkNode`, but a `LinkNode` listener never sees it,
    so the policy registers for both. Any new link subclass needs the same.
- Lexical's stock `AutoLinkPlugin` was rejected. It only accepts `.,;` and
  whitespace as boundaries, so `(https://x)` never links.

## Read-only is not "no markdown sync"

`disabled` editors still export through `MarkdownSyncPlugin`'s `onChange`:

- Issue descriptions wire `onChange` → `onFormChange` while only displayed.
- Composers (chat box, comments, approvals, retry) are `disabled` only while
  a send is in flight.

Splitting formatted text around a link is **not byte-identical** on
markdown export: `**bold https://b.io**` becomes `**bold&#32;https://b.io**`.
Any model change made only for display can therefore rewrite stored content.
The rules that keep it out:

1. Display-only changes run in their own update tagged
   `AUTO_LINK_UPDATE_TAG`, and `MarkdownSyncPlugin` does not export those
   updates. A `TextNode` transform does not work for this, because
   transforms run inside the update that dirtied the node (the content
   parse), so the change gets exported with it.
2. **Lexical merges the tags of updates batched into one commit.** An update
   queued in the same tick as the content parse (in a mount effect, say)
   tags the parse as well and hides its `onChange`. Queue display-only
   updates in a microtask so they commit separately.
3. Tagging the one update is not enough: the split nodes stay in the tree,
   and any later untagged update re-exports them. A reader selecting text is
   enough. So `MarkdownSyncPlugin` also skips export while the editor is
   **not editable** and the tree holds `AutoLinkNode`s. A fresh parse never
   contains them, so real value changes still sync.
4. Keep that guard scoped to read-only. Lexical's clipboard format carries
   `AutoLinkNode`s from a rendered message into an editable composer
   (both use the `md-wysiwyg` namespace). There they are user content, and
   an unconditional guard would silence the composer's `onChange`.
5. When the editor becomes editable (plugin unmount), unwrap every
   `AutoLinkNode`. Lexical's normalization merges the text back, so editing
   starts from the original node structure.

Each of these guards has a test in
`packages/web-core/src/shared/components/ReadOnlyAutoLinkPlugin.test.tsx`
that fails when the guard is removed. The tests render through
`MarkdownSyncPlugin`, just as `WYSIWYGEditor` does, and compare `onChange`
output with an editor that never auto-links.

## Verification notes

- `packages/ui` formats with `--config ../../packages/local-web/.prettierrc.json`.
  A bare `prettier` call on `ui` files falls back to double quotes and
  rewrites every line.
- The local-web ESLint config cannot parse web-core `*.test.tsx` files, which
  are outside the tsconfig. The "Parsing error" on them is expected and
  pre-existing.

## Contributed by

- vk/e4ef-urls-always-clic
