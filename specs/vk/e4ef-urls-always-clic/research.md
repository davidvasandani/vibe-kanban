# Research: URLs always clickable

## Decision: custom linking pass, not Lexical's `AutoLinkPlugin`
`@lexical/react/LexicalAutoLinkPlugin` (via `registerAutoLink` in
`@lexical/link` 0.36) only links a match whose neighbouring characters match
`/[.,;\s]/` (`isContentAroundIsValid`). So `(https://x)`, `"https://x"` and
`<https://x>` would not link. That fails FR-4, and agents write all three
often. Its matcher contract also gives no control over trailing-paren
balancing. A small linking pass with our own boundary rules is less code than
working around it. It reuses `AutoLinkNode` / `$createAutoLinkNode` from
`@lexical/link`, which is already a dependency.

## Decision: `AutoLinkNode` rather than `LinkNode`
The markdown `LINK` export transformer in `@lexical/markdown` 0.36 skips
`AutoLinkNode` (`if (!$isLinkNode(node) || $isAutoLinkNode(node)) return
null`). A detected URL therefore serialises back as its plain text. A
`LinkNode` would export as `[url](url)` and rewrite stored markdown in any
surface that wires `onChange`.

## Decision: allow `http://`
Upstream limited clickable links to `https://`. The user's request is that
URLs are "always" clickable, and homelab services and dev servers are often
plain HTTP. The security-relevant property is the scheme allow-list
(`javascript:`, `data:` and the rest stay inert), which is unchanged.

## Decision: trailing punctuation rules
Trim `. , ; : ! ? ' " *` and backtick from the end, repeatedly. Trim `) ] }`
only while the URL contains more closers than openers of that kind. This
matches what GitHub's autolinker does for Wikipedia-style `a_(b)` paths.
`*`, `_` and `~` would only appear when markdown emphasis failed to parse.
`_` and `~` are legal at the end of a URL, so only `*` is trimmed.

## Alternatives rejected
- **Pre-processing markdown text into `[url](url)` before import.** It would
  have to skip code spans and fences by re-implementing a markdown tokenizer,
  and export would no longer round-trip.
- **DOM-level linkify after render (like `ClickableCodePlugin`).** It would
  fight Lexical reconciliation, which re-creates DOM nodes, and could not be
  tested against the model.

## Decision: tagged updates instead of a node transform (made during implementation)
The first version used a `TextNode` transform. Transforms run inside the
update that dirtied the node, so the linked tree reached
`MarkdownSyncPlugin`'s `onChange`. A test showed the export was not
byte-identical (`**bold https://b.io**` became `**bold&#32;https://b.io**`).
Issue descriptions wire `onChange` while read-only, so that could rewrite a
description. Linking now runs in its own update tagged
`AUTO_LINK_UPDATE_TAG`, which `MarkdownSyncPlugin` does not export. Lexical
merges the tags of updates batched into one commit, so the mount and unmount
passes are queued with `queueMicrotask`. Otherwise they would hide the
initial parse's `onChange`, as an earlier test run showed.
