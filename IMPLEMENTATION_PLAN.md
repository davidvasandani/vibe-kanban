# Implementation plan: bare URLs always clickable

Task `vk/e4ef-urls-always-clic`. See `SPEC.md` for the design and
`PRIOR_KNOWLEDGE.md` for the link rules inherited from PR #313.

## Step 1: URL detection helper and plugin (`packages/ui`)

File: `packages/ui/src/components/ReadOnlyAutoLinkPlugin.tsx` (new)

1. `export function findUrlMatches(text): { start, end, url }[]`
   - Scan with `/https?:\/\/[^\s<>]+/gi`.
   - Trim trailing characters from `.,;:!?'"*` and backticks.
   - Trim a trailing `)`, `]` or `}` only when the candidate has more closers
     than openers of that kind. Loop until stable.
   - Drop the match if only the scheme is left (`https://`).
2. `ReadOnlyAutoLinkPlugin()` links text in updates tagged
   `AUTO_LINK_UPDATE_TAG` (plus `HISTORY_MERGE_TAG`). It runs once on mount,
   and then from an update listener over the dirty leaves of any update that
   does not carry the tag.
   - `$linkUrls(node)`: skip the node if it is not simple text, or if an
     ancestor is a link or a `CodeNode`. Otherwise split each match out, wrap
     it in `$createAutoLinkNode(url)`, and continue on the tail.
   - On unmount (the editor becomes editable), unwrap every `AutoLinkNode`.
   - Queue the mount and unmount updates with `queueMicrotask`, so they never
     batch with a content update and hide it behind the tag.
   - Throw if `AutoLinkNode` is not registered on the editor.
3. `MarkdownSyncPlugin.tsx`: do not call `onChange` for updates tagged
   `AUTO_LINK_UPDATE_TAG`. Splitting formatted text is not byte-identical on
   export (`**a https://b**` gains `&#32;`). Issue descriptions wire
   `onChange` while read-only, and composers are `disabled` while sending.

## Step 2: allow `http` and auto links (`packages/ui`)

File: `packages/ui/src/components/ReadOnlyLinkPlugin.tsx`

1. Change the external check to `/^https?:\/\//i`.
2. Register the same mutation listener for `AutoLinkNode`, because mutation
   listeners are per class. Update the comments.

## Step 3: wire into the editor (`packages/web-core`)

File: `packages/web-core/src/shared/components/WYSIWYGEditor.tsx`

1. Add `AutoLinkNode` to `nodes`.
2. Render `{disabled && <ReadOnlyAutoLinkPlugin />}` next to `ReadOnlyLinkPlugin`.

## Step 4: tests (`packages/web-core`)

- `ReadOnlyAutoLinkPlugin.test.tsx` (new, jsdom). It runs
  `findUrlMatches` table tests and renders markdown through `LexicalComposer`
  with both plugins, checking:
  - the screenshot sentence
  - parentheses and quotes
  - multiple URLs
  - bold text
  - code blocks are not linked; inline code is linked and keeps its code format
  - existing markdown links are left alone
  - an `http` URL is clickable
  - the markdown export round-trips
- `ReadOnlyLinkPlugin.test.tsx`: update the `http` expectations, which were
  disabled and are now clickable.

## Step 5: verify

`pnpm --filter @vibe/web-core exec vitest run`, `pnpm run check`,
`pnpm run lint`, `pnpm run format`.
