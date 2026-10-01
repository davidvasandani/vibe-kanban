# SPEC: Bare URLs are always clickable in rendered markdown

Task: `vk/e4ef-urls-always-clic`.

## Problem

Agent replies often contain bare URLs, for example "pushed to PR #1803:
https://github.com/sweetgreen/terraform-infrastructure/pull/1803." The chat
renders them as plain text. Users have to select and copy them; they cannot
click them.

Read-only markdown (conversation entries, issue descriptions, comments,
approvals, notes) is rendered by `WYSIWYGEditor` in `disabled` mode. That
uses Lexical's markdown import, and the `LINK` transformer only recognises
`[text](url)`. A bare URL stays a `TextNode`. `ReadOnlyLinkPlugin` only sees
`LinkNode`s, so it never runs on bare URLs.

There is a second gap: `ReadOnlyLinkPlugin` makes only `https://` links
clickable. Markdown links to `http://` destinations, such as
`http://localhost:3000` dev servers or LAN addresses, render as disabled.

## Goals

1. In read-only rendering, every bare `http://` or `https://` URL in normal
   prose becomes a clickable link. That covers paragraphs, list items,
   headings, quotes, table cells, and bold or italic text.
2. Links open in a new tab with `rel="noopener noreferrer"`, the same as
   existing external links.
3. Trailing sentence punctuation is not part of the link: `.`, `,`, `;`,
   `:`, `!`, `?`, quotes, and closing brackets with no matching opener. So
   `(see https://x.y/a_(b))` links `https://x.y/a_(b)`, and `https://x.y.`
   links `https://x.y`.
4. URLs that directly follow non-space punctuation are linked too, such as
   `(https://…)`, `"https://…"` and `<https://…>`. Lexical's stock
   `AutoLinkPlugin` does not handle these, because it only treats `.,;` and
   whitespace as boundaries.
5. Explicit markdown links to `http://` destinations are clickable as well.
6. Some text is left alone:
   - fenced code blocks. Inline code is still linked, and the link wraps the
     code-styled text.
   - text that is already inside a link
   - anything whose scheme is not `http` or `https`. `javascript:`, `data:`
     and the rest are still never clickable.
7. Editing mode is unchanged: no autolinking while the user is composing.
8. Stored markdown is unchanged. Linking never reaches `onChange`, and the
   links are removed when the editor becomes editable (see Design).

## Non-goals

- `www.example.com` without a scheme, and email addresses.
- `SimpleMarkdown` and `RawLogText`, which already linkify with their own
  regexes.
- Client-side routing for links that point into the app.

## Design

- A new `ReadOnlyAutoLinkPlugin` in `packages/ui/src/components/`. It
  links text in its own Lexical update tagged `AUTO_LINK_UPDATE_TAG`. That
  update runs once on mount, and again for the dirty leaves of every later
  update that does not carry the tag.
  - It skips nodes that are not simple text, text whose parent is a link,
    and text inside a `CodeNode`. Inline code is simple text with a `code`
    format, so it is still linked.
  - It scans the text with `/https?:\/\/[^\s<>]+/gi`, trims trailing
    punctuation and unbalanced closers, and then splits the node. Each URL
    slice is wrapped in an `AutoLinkNode` and keeps its text format. It loops
    over the remainder, so several URLs in one node are all linked.
  - A `findUrlMatches(text)` helper is exported so it can be unit-tested.
- Why the update is tagged and not a node transform: read-only editors still
  sync markdown out. Issue descriptions wire `onChange` while displayed, and
  composers are `disabled` while sending. Splitting formatted text is not
  byte-identical on export (`**a https://b**` gains `&#32;`), so linking must
  never reach `onChange`. `MarkdownSyncPlugin` skips updates carrying the
  tag, and it also skips any update while the editor is read-only and the
  tree still holds auto links. A selection is enough to trigger such an
  update. An editable tree is exempt, because an `AutoLinkNode` pasted into a
  composer is user content. The mount and unmount updates are queued in a
  microtask, because Lexical merges the tags of updates batched into one
  commit, and an untagged content change such as the initial parse would
  otherwise be hidden.
- On unmount, when the editor becomes editable, every `AutoLinkNode` is
  unwrapped. Lexical's normalization merges the text back together, so
  editing starts from the original node structure.
- `WYSIWYGEditor` registers `AutoLinkNode` and mounts
  `ReadOnlyAutoLinkPlugin` only when `disabled`, next to
  `ReadOnlyLinkPlugin`.
- `ReadOnlyLinkPlugin` also listens to `AutoLinkNode` mutations, because
  Lexical mutation listeners match exact node classes. Its external check
  becomes `/^https?:\/\//i`.

## Acceptance

- The screenshot case: a bare GitHub PR URL followed by `.` renders as an
  `<a href=… target=_blank>` without the period.
- Vitest covers the punctuation, parenthesis, code, existing-link, multiple-URL
  and `http` cases, and the markdown round-trip.
- `pnpm run check`, `pnpm run lint` and the web-core vitest suite pass.
