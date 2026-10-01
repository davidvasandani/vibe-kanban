# Implementation Plan: URLs always clickable

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context
- Frontend only. The stack is React 18, TypeScript and Lexical `^0.36.2`
  (`lexical`, `@lexical/link`, `@lexical/markdown`, `@lexical/react`, all
  already dependencies of `packages/ui` and `packages/web-core`).
- Every read-only markdown surface (conversation entries, issue
  descriptions, comments, approvals, notes, review comments) renders through
  `packages/web-core/src/shared/components/WYSIWYGEditor.tsx` with
  `disabled`. That path parses markdown with `$convertFromMarkdownString` in
  `packages/ui/src/components/MarkdownSyncPlugin.tsx`.
- Link safety lives in `packages/ui/src/components/ReadOnlyLinkPlugin.tsx`.
  It is a `LinkNode` mutation listener that allow-lists hrefs and disables
  everything else.
- No backend, schema or generated-type changes.

## Architecture & Approach
1. **Detection: `packages/ui/src/components/ReadOnlyAutoLinkPlugin.tsx`
   (new).**
   - `findUrlMatches(text)` is a pure function. It finds
     `https?://[^\s<>]+` with no left-boundary requirement (FR-4), trims
     trailing punctuation and quotes, and removes trailing closers only while
     they are unbalanced (FR-3). It returns all matches (FR-5).
   - `ReadOnlyAutoLinkPlugin` links text in its own updates, tagged
     `AUTO_LINK_UPDATE_TAG`. It runs once on mount, and then for the dirty
     leaves of every update that does not carry the tag. For a simple text
     node that is not inside a link and not inside a `CodeNode` (FR-7), it
     splits each match out and wraps it in `$createAutoLinkNode(url)`. The
     text keeps its format (bold or inline `code`). Fenced code is made of
     `CodeHighlightNode`s, which are not simple text, and the `CodeNode`
     ancestor check is a second guard.
   - It is mounted only when `disabled` (FR-9). Read-only editors still sync
     markdown out: issue descriptions wire `onChange`, and composers are
     `disabled` while sending. Splitting formatted text is also not
     byte-identical on export (`**a https://b**` gains `&#32;`). So
     `MarkdownSyncPlugin` skips `onChange` for tagged updates, the plugin
     unwraps every auto link on unmount, and the mount and unmount updates
     are queued in a microtask so that Lexical's tag-merging of batched
     updates cannot hide a real content change.
2. **Clickability: `ReadOnlyLinkPlugin.tsx`.**
   - Widen the external test to `/^https?:\/\//i` (FR-6, FR-8).
   - Register the existing listener for `AutoLinkNode` as well, because
     Lexical mutation listeners are keyed by exact class. Auto links then get
     `target=_blank` and `rel=noopener noreferrer` (FR-2). The issue-route
     branch is untouched (FR-10).
3. **Wiring: `WYSIWYGEditor.tsx`.** Add `AutoLinkNode` to `nodes`, and
   render `{disabled && <ReadOnlyAutoLinkPlugin />}` before
   `ReadOnlyLinkPlugin`.
4. **Tests: `packages/web-core/src/shared/components/`.**
   - `ReadOnlyAutoLinkPlugin.test.tsx` (new). It runs `findUrlMatches`
     tables, plus rendered `LexicalComposer` cases with the markdown
     `TRANSFORMERS`, `CodeNode` and both plugins, covering every acceptance
     criterion including the export round-trip.
   - `ReadOnlyLinkPlugin.test.tsx`: `http://` destinations are now expected
     to be clickable.

## Data Model
Not applicable. Nothing is persisted. `AutoLinkNode` exists only in the
in-memory Lexical tree of read-only editors.

## Contracts
Not applicable. There are no API or interface changes beyond the exported
`findUrlMatches` / `ReadOnlyAutoLinkPlugin` from `@vibe/ui`.

## Research Notes
See `./research.md`.

## Constitution Check
- I (clarity): one small plugin plus a pure helper, commented where it is not
  obvious (the class-keyed mutation listener, the export skip).
- II (test the contract): there are rendered-DOM tests for every acceptance
  criterion, and the regex is tested separately (XLV asks for both).
- III (small steps): reuses Lexical's `AutoLinkNode` and the existing
  `ReadOnlyLinkPlugin` policy rather than a second sanitiser.
- IV (shared boundaries): the plugin lives in `packages/ui` next to its
  siblings, and wiring stays in the `web-core` editor. Both `local-web` and
  `remote-web` get the change. Nothing in it is host-specific.
- XLV: the scheme allow-list, read-only-only detection, skipping fenced
  blocks, inline code linked, and the round-trip are all satisfied.
- No new dependencies (Constraints).

## Risks & Dependencies
- **Export drift (confirmed during implementation).** A test showed
  `**bold https://b.io**` exporting as `**bold&#32;https://b.io**` once split.
  This is mitigated by the tag guard. A test checks that `onChange` output
  matches an editor that never auto-links, and fails if the guard is removed.
- **Tag merging (confirmed).** Linking synchronously on mount merged with
  the initial parse and hid its `onChange`. Queuing in a microtask fixes it,
  and the same test covers it.
- **Performance.** Each pass looks only at dirty leaves: one regex pass per
  changed text node.
- **Disabled → enabled toggle.** Unmount unwraps every auto link, so editing
  starts from whole text nodes. A test checks this, and fails if the unwrap
  is removed.
- **Untagged updates while links exist.** Any other read-only update, such
  as an image node refreshing, would export the split nodes. In read-only
  mode the content updates are re-parses, which replace the tree, so this is
  accepted.

## Review outcome (Codex, three passes)
1. **P2: later untagged updates exported the split tree.** A read-only
   selection is enough to trigger one, and on an issue description that
   would rewrite it through `onFormChange`. Fix: `MarkdownSyncPlugin` also
   skips `onChange` while the editor is read-only and the tree holds
   `AutoLinkNode`s (`$hasAutoLinks`). The test now runs selection and
   dirtying updates after linking.
2. **P1: that guard silenced editable composers.** Pasting a rendered message
   carries its `AutoLinkNode` through Lexical's clipboard format. Fix: the
   guard applies only when `!editor.isEditable()`. In an editable tree an auto
   link is user content and exports as its URL. Covered by the "pasted auto
   link" test.
3. The third pass found nothing to fix. Each guard has a test that fails
   when the guard is removed.
