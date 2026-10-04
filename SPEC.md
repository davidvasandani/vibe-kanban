# SPEC: Start stopped Session UI

Task: `vk/556e-start-stopped-se`. Feature spec, plan and tasks:
`specs/vk/556e-start-stopped-se/`.

## Problem

VAS-780 attaches a reference photo of a "Stopped" status pill next to a big
"Continue" button, and asks for three structural changes to the session UI
that represents a stopped run:

1. Remove the bottom buttons.
2. Remove the top continue button.
3. Move the icon buttons to where the top continue button was (left of the
   "stopped" label), and add a start/restart button.

The reference photo is not a screenshot of this app (its chrome —
"Pinned chats", a row of "Slack" tabs — matches no string anywhere in this
repo; see `PRIOR_KNOWLEDGE.md`). The issue explicitly permits discarding its
literal wording once the real current UI is identified. The only existing
affordance in this codebase for "a session that stopped and can be resumed"
is the *interrupted-run banner* in
`packages/ui/src/components/SessionChatBox.tsx`: when the latest
coding-agent process for a session was killed by a VK server restart
(`ExecutionProcessStatus.interrupted`), the composer shows a banner reading
"This run was interrupted by a vibe-kanban restart." with a secondary
"Resume" button. That banner sits at the *top* of the composer
(`ChatBoxBase`'s `banner` slot, above the header/editor); the composer's
*bottom* footer row (`footerLeft`/`footerRight`) separately renders icon-only
buttons (attach file, insert GitHub PR comments, and any `toolbarActions`
items) plus the status-driven action button (`Send`, when idle).

## Decision

Apply the three instructions to this banner/footer pair, which is the only
component matching "a stopped-session indicator with a continue button above
a row of icon buttons and other buttons below":

- The footer's icon-only buttons (`footerLeft`: attach file, GitHub PR
  comments, `toolbarActions`) move into the banner row, to the left of the
  status label, **only while the banner is shown** (i.e. only for this
  stopped/interrupted session). They are removed from their old spot in the
  footer at the same time — this single move satisfies both "remove the
  bottom buttons" and "move the icon buttons ... left of 'stopped'": the
  buttons are not duplicated, they relocate.
- The banner's existing "Resume" button (the "top continue button") is
  removed and replaced by a new button whose label and intent is
  start/restart (not "resume"): it re-sends the same interrupted-run
  continuation prompt as before (unchanged handler/behavior), under new
  wording.
- The footer's `footerRight` action button (`Send`, driven by `status`) is
  **not** touched — it is not "icon buttons" and the issue's instructions
  map cleanly onto the icon-button row without it. Removing the ability to
  send a normal follow-up message while interrupted is a materially bigger
  behavior change than "move some buttons," and the issue does not ask for
  it.
- The banner's long explanatory sentence collapses to a short status label
  ("Stopped"), matching the terse pill-style wording the issue references;
  the fuller explanation moves to a `title` tooltip so the information is
  not lost.

## Non-Goals

- Changing when the interrupted banner appears (still driven by
  `hasInterruptedLatestProcess` / `ExecutionProcessStatus.interrupted`).
- Changing the `Send` button, the editor, or any other execution status
  (`running`, `queued`, `stopping`, etc.) — only the interrupted banner and
  the icon-button row's position change, and only while that banner shows.
- Introducing a dedicated "stopped session card" component outside
  `SessionChatBox`/`ChatBoxBase` — no other component in the frontend
  currently renders a stopped-session indicator (see `PRIOR_KNOWLEDGE.md`).
- Changing `ExecutionProcessStatus` values, backend behavior, or the
  resume/continuation prompt text (`RESUME_INTERRUPTED_PROMPT`).

## Functional Requirements

- **FR-001**: While a session's latest coding-agent process is interrupted,
  the composer MUST show one consolidated row (replacing today's banner)
  containing, left to right: the icon buttons formerly in the footer
  (attach file; GitHub PR-comment insert, when available; any
  `toolbarActions` items), a short "Stopped" status label, and a start/
  restart button.
- **FR-002**: The footer (`footerLeft`) MUST NOT render those icon buttons
  while the interrupted banner is shown — they appear only once, in the
  banner row.
- **FR-003**: When the interrupted banner is not shown (no interrupted
  process, or a different session), the footer MUST render the icon buttons
  exactly as before — unchanged behavior, unchanged position.
- **FR-004**: The new start/restart button MUST invoke the same resume
  behavior as today's "Resume" button (same handler, same
  `RESUME_INTERRUPTED_PROMPT` continuation, same `isResuming`/disabled
  treatment while in flight).
- **FR-005**: The footer's `footerRight` action button (`Send` in idle
  status) MUST be unaffected — still rendered, unchanged position and
  behavior, in both interrupted and non-interrupted states.
- **FR-006**: All new or changed user-facing strings MUST go through
  `react-i18next` (`t(...)`), not literal JSX text, and every one of the 7
  locale files (`en es fr ja ko zh-Hans zh-Hant`) under
  `packages/web-core/src/i18n/locales/` MUST define the same key set
  afterward (`scripts/check-i18n.sh` enforces this in CI).
- **FR-007**: Any i18n key removed from `en` (e.g. the old `resume`/
  `resuming` keys, if fully superseded) MUST be removed from all 7 locales
  too, and MUST NOT remain referenced anywhere in source
  (`scripts/check-unused-i18n-keys.mjs` enforces this).

## Validation

- `pnpm run check` (frontend + Rust type checks).
- `pnpm run lint` (ESLint across `local-web`/`web-core`/`remote-web`/`ui`,
  clippy, and the unused-i18n-key check).
- `bash scripts/check-i18n.sh` locally if feasible (it also runs in CI).
- A focused Vitest test for `SessionChatBox` (or its container) asserting:
  icon buttons render once, in the banner, while interrupted; the footer's
  icon-button slot is empty while interrupted; the restart button triggers
  the existing resume callback; non-interrupted sessions keep the icon
  buttons in the footer.
- Manual verification in the running dev app (`pnpm run dev`): find or
  produce a session with an interrupted latest process, confirm the new
  single-row layout, confirm restart resumes the session, confirm the `Send`
  button still works, confirm a normal (non-interrupted) session is
  unchanged.
