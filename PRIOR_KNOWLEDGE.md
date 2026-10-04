# Prior knowledge: Start stopped Session UI (`vk/556e-start-stopped-se`)

Distilled from `wiki/` (the project knowledge base). Read-only recall.

## The "interrupted" banner is the existing stopped-session affordance

`packages/ui/src/components/SessionChatBox.tsx` renders an inline banner
when the latest coding-agent process for a session has
`ExecutionProcessStatus.interrupted` (killed by a VK server restart, not a
normal completion). `wiki/coordinator-restart-handoff.md` confirms this is
the canonical UI for a stopped-but-resumable run: "It is still `Interrupted`
+ WIP commit + the opt-in `resume_interrupted_on_startup`... the chat shows
Resume." The banner is computed in
`packages/web-core/src/features/workspace-chat/ui/SessionChatBoxContainer.tsx`
(`hasInterruptedLatestProcess`, `handleResumeInterrupted`) and rendered by
`ChatBoxBase`'s `banner` slot — the topmost row of the composer, above the
header, editor and footer. `wiki/agent-process-lifecycle.md` has adjacent
detail on the one-turn-one-`ExecutionProcess` identity chain and why process
liveness alone isn't turn evidence for non-natural-exit executors.

## `ChatBoxBase` layout: banner (top) vs. footer (bottom)

`packages/ui/src/components/ChatBoxBase.tsx` composes, top to bottom: error
alert, `banner`, header (`headerLeft`/`headerRight`, only in
`VisualVariant.NORMAL`), editor, then a `footer` row with `footerLeft`
(icon-only `ToolbarIconButton`s: attach file, GitHub PR-comment insert, and
any `toolbarActions` items) and `footerRight` (the status-driven action
button(s) from `renderActionButtons()` in `SessionChatBox.tsx`). This is the
only place in the frontend where an icon-button row sits structurally below
a single-button banner row — i.e. the only real candidate for "icon buttons"
+ "bottom buttons" + a banner-level action button in one composer.

## No literal "Stopped"/"Continue" pair exists in the codebase

Exhaustive grep across `packages/{ui,web-core,local-web,remote-web}/src`
(`.tsx`, `.ts`, and every i18n locale `.json`) found no component or string
table rendering a status pill literally reading "Stopped", and no button
literally reading "Continue" for session resumption. The closest real
affordance is `tasks.json`'s `conversation.interrupted.{message,resume,resuming}`
("Interrupted" / "Resume" / "Resuming"), used only by the banner above. The
`IssueWorkspaceCard`/`WorkspaceSummary` components show `Active`/`Archived`
or raw elapsed-time + status-icon rows, never a "Stopped" word; the kanban
board, right sidebar, and context bar have no matching strings either. The
issue's reference screenshot is almost certainly an external/mockup image
(unrelated app chrome — "Pinned chats" matches no string in this repo), not
a literal screenshot of vibe-kanban; the issue explicitly authorizes
discarding its wording once the real UI is found.

## i18n CI gates (must hold for any wording/layout change here)

- `scripts/check-i18n.sh`: (1) no new `i18next/no-literal-string` ESLint
  violations versus `main`, so any new/renamed label needs a `t()` key, not
  a literal; (2) every namespace JSON under
  `packages/web-core/src/i18n/locales/<lang>/` must have exactly the same
  key set as the `en` counterpart — all 7 locales (`en es fr ja ko zh-Hans
  zh-Hant`) move together; (3) no duplicate JSON keys.
- `scripts/check-unused-i18n-keys.mjs`: every leaf key under `en` must be
  referenced somewhere in `packages/{web-core,local-web,remote-web,ui}/src`.
  Renaming/removing `conversation.interrupted.resume`/`resuming` means
  deleting them from all 7 locale files (not just `en`) once their last use
  is gone, and adding replacement keys to all 7.

## Gaps (no prior knowledge)

- No wiki page documents a "stopped session card" with an icon-button row
  distinct from the composer footer, confirming this is new ground, not a
  rename of something already designed elsewhere.
