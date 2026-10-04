# Implementation Plan: Start stopped Session UI

**Spec**: `./spec.md` | **Constitution**: `.specify/memory/constitution.md`
(review appended: "Review: vk/556e-start-stopped-se" — no amendment)

## Summary

`SessionChatBox` (`packages/ui/src/components/SessionChatBox.tsx`) already
has everything this feature needs: an `interruptedNotice` banner slot
(`renderBanner`) and a footer icon-button group (`footerLeft`). The plan is
a pure reshuffle — extract the footer's icon buttons into a shared closure,
render that closure in the banner instead of the footer while
`interruptedNotice` is set, and swap the banner's "Resume" button for a
"Restart" one with the same handler. No new component, no backend change,
no prop-contract change (`interruptedNotice`'s shape is untouched).

## Technical Context

- **Language/Framework**: TypeScript, React, Tailwind (`packages/ui`).
- **Owning package**: `packages/ui` (per constitution IV, this is the only
  package that may change the chat box's internal layout; both
  `local-web` and `remote-web` consume it unchanged).
- **Testing**: Vitest + Testing Library, test file lives in
  `packages/remote-web/src/test/` per existing repo convention (see
  `SessionChatBox.test.tsx`), not colocated with the component.
- **i18n**: `react-i18next`, namespace `tasks`, key path
  `conversation.interrupted.*` in
  `packages/web-core/src/i18n/locales/<lang>/tasks.json` for all 7 locales.

## Design

### `SessionChatBox.tsx`

1. Add a local `renderIconButtons()` closure (defined once, near
   `handleAttachClick`) containing exactly what `footerLeft` renders today:
   the attach-file `ToolbarIconButton`, the GitHub PR-comment
   `ToolbarIconButton` (gated on `onPrCommentClick`), and the
   `toolbarActions?.items.map(...)` icons — same props, same disabled
   logic, unchanged.
2. `footerLeft` becomes `{!interruptedNotice && renderIconButtons()}` plus
   the always-mounted hidden `<input type="file">` (unconditional — it has
   no visible position requirement and `handleAttachClick` must keep
   finding it via `fileInputRef`).
3. In `renderBanner()`'s interrupted branch, replace the
   `WarningIcon`/message/`Resume`-button row with:
   `renderIconButtons()` → status `<span>` (new `status` key, `title`
   carries the old `message` key so the explanation isn't lost) → a
   `flex-1` spacer → a `PrimaryButton` wired to
   `interruptedNotice.onResume`/`isResuming`, labeled with new
   `restart`/`restarting` keys and an `ArrowsClockwiseIcon` action icon
   (imported from `@phosphor-icons/react`, already used elsewhere in this
   codebase for a "regenerate" affordance).

No other file in `packages/ui`, `packages/web-core`, `packages/local-web`,
or `packages/remote-web` renders this banner/footer pair, so no other
caller needs updating (`SessionChatBoxContainer.tsx` passes
`interruptedNotice`/`toolbarActions` through unchanged — confirmed by
reading its construction of both props).

### i18n

`packages/web-core/src/i18n/locales/{en,es,fr,ja,ko,zh-Hans,zh-Hant}/tasks.json`,
`conversation.interrupted` block: add `status`, `restart`, `restarting`;
keep `message`; remove `resume`/`resuming` once no longer referenced
(confirmed via `node scripts/check-unused-i18n-keys.mjs`).

## Research / Alternatives considered

- **Hide the editor + footer entirely while interrupted, not just the icon
  buttons** (footer's `Send` button also disappears): rejected — `Send`
  already works as an alternative to Resume today (typing and sending
  bypasses the interrupted banner entirely), and the issue's instructions
  map cleanly onto only the icon-button row without touching `Send`. Taking
  away `Send` would be a materially larger, unrequested behavior change
  (constitution III: smallest change that delivers the value).
- **A new standalone "stopped session" component**, separate from
  `SessionChatBox`: rejected — no other component in the frontend renders a
  stopped-session indicator (`PRIOR_KNOWLEDGE.md`), and `SessionChatBox`
  already owns exactly this banner; constitution VI (don't rebuild what
  shipped) and IV (packages/ui owns this surface) both point at extending
  the existing component.
- **Duplicate the icon buttons in both places** instead of conditionally
  relocating them: rejected — the issue says "move," and duplicate action
  buttons with the same handlers would be confusing and untestable as "the
  same button."

## Complexity Tracking

No constitution principle violated; no new top-level dependency; no new
component. Nothing to justify here.
