# Implementation plan: Start stopped Session UI

Task `vk/556e-start-stopped-se`. Spec: `SPEC.md`. SpecKit:
`specs/vk/556e-start-stopped-se/`.

## 1. `packages/ui/src/components/SessionChatBox.tsx`

- Extract the footer icon-button group (currently inline in `footerLeft`:
  the attach-file `ToolbarIconButton`, the GitHub PR-comment
  `ToolbarIconButton` gated on `onPrCommentClick`, and the
  `toolbarActions?.items.map(...)` icons) into a local `renderIconButtons()`
  closure inside the component, reusing the same handlers/disabled logic
  unchanged.
- Render `renderIconButtons()` in `footerLeft` only `!interruptedNotice`.
  Keep the hidden `<input type="file">` mounted unconditionally (it has no
  visual position requirement).
- Rewrite the "Interrupted-run banner" branch in `renderBanner()`: replace
  the `WarningIcon` + long message + secondary "Resume" `PrimaryButton` with
  one row: `renderIconButtons()`, then a short status `<span>` (new key,
  see below) carrying the old message as `title`, then a `flex-1` spacer,
  then a `PrimaryButton` wired to `interruptedNotice.onResume` /
  `interruptedNotice.isResuming` with the new restart wording.
- `InterruptedNoticeProps`/prop name `interruptedNotice` stay as-is (no
  reason to churn the public prop contract); only the rendered label and
  layout change.

## 2. i18n — `packages/web-core/src/i18n/locales/*/tasks.json`

In the `conversation.interrupted` block, across all 7 locales
(`en es fr ja ko zh-Hans zh-Hant`):

- Add `"status"`: short label ("Stopped" / localized equivalent).
- Add `"restart"`: replaces `"resume"` ("Restart" / localized equivalent).
- Add `"restarting"`: replaces `"resuming"` ("Restarting..." / localized
  equivalent).
- Keep `"message"` (the full sentence) — now used as the status `title`
  tooltip instead of inline body text.
- Remove `"resume"`/`"resuming"` once nothing references them (confirm with
  `node scripts/check-unused-i18n-keys.mjs --list`).

Translations: use straightforward, idiomatic equivalents consistent with
each locale's existing short-label style (e.g. existing `buttons.*` /
status-word entries in the same files) rather than literal machine
translation of the English phrasing.

## 3. Tests

Add or extend a Vitest test (likely
`packages/ui/src/components/SessionChatBox.test.tsx` if one exists, else a
new file beside it, following this repo's existing RTL conventions) that:

- Renders `SessionChatBox` with `interruptedNotice` set and asserts the icon
  buttons (attach file, and a sample `toolbarActions` item) render inside
  the banner row and do NOT render in the footer.
- Asserts clicking the new restart button calls `interruptedNotice.onResume`.
- Renders `SessionChatBox` without `interruptedNotice` and asserts the icon
  buttons render in the footer (unchanged from current behavior) and no
  banner row is present.

## 4. Validation

1. `pnpm run format`.
2. `pnpm run check`.
3. `pnpm run lint` (includes the unused-i18n-key check and clippy; no Rust
   changes in this task, but the workspace-wide `cargo clippy` still runs).
4. `bash scripts/check-i18n.sh` (or at minimum manually diff locale key sets
   — the script needs network access to clone `main`, which may not be
   available in this environment; if it fails for that reason only, verify
   key-set equality by hand instead of treating it as a code regression).
5. Manual check in the running dev app: produce or find a session with an
   interrupted latest process (e.g. restart the dev backend mid-run, or
   inspect an existing workspace whose latest process status is
   `interrupted`), confirm the new layout, confirm restart works, confirm
   non-interrupted sessions are unchanged.

## 5. Knowledge base

After landing, add/update a `wiki/` page (or extend
`coordinator-restart-handoff.md`, which already documents "the chat shows
Resume") noting the banner's new consolidated layout and that the icon
buttons move into it conditionally — so a future task touching either the
footer or the interrupted banner knows they now interact.
