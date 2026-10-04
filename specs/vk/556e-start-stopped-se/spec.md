# Feature Specification: Start stopped Session UI

**Feature Branch**: `vk/556e-start-stopped-se`
**Created**: 2026-10-04
**Status**: Specified
**Input**: VAS-780 — "1. Remove the bottom buttons. 2. Remove the top
continue button. 3. Move the icon buttons to where we removed the top
continue button, left of 'stopped' and add a start/restart button."
(reference photo attached; its exact wording/chrome does not match this
app — see `PRIOR_KNOWLEDGE.md` — so the issue's own text authorizes
mapping its structural intent onto the real current UI rather than
reproducing the photo literally)

## User Scenarios & Testing

### Primary User Story

As a user whose coding-agent session was stopped by a VK server restart, I
see one compact row telling me it stopped and offering to restart it, with
the same utility icon buttons I'd normally reach for (attach a file, insert
PR comments, etc.) available right there — instead of a sentence-plus-button
banner on top and a separate, now-redundant row of icon buttons and a send
button underneath.

### Acceptance Scenarios

1. **Given** a session whose latest coding-agent process was interrupted by
   a server restart, **When** its chat box renders, **Then** it shows one
   row with: the icon buttons (attach file; GitHub PR-comment insert, when
   available; any extra toolbar actions), a short "Stopped" label, and a
   restart button — and no separate footer row below it.
2. **Given** that same stopped-session row, **When** the user clicks the
   restart button, **Then** the session resumes exactly as today's "Resume"
   button did (same prompt, same in-flight/disabled treatment).
3. **Given** a session whose latest process is not interrupted (running,
   idle, queued, etc.), **When** its chat box renders, **Then** the icon
   buttons and the status-driven action button (e.g. Send) appear in the
   footer exactly as before — unchanged.
4. **Given** the stopped-session row, **When** the user inspects the status
   label, **Then** the original explanatory sentence ("This run was
   interrupted by a vibe-kanban restart.") is still available (as a
   tooltip), not deleted.

### Edge Cases

- A `toolbarActions` item is absent (no extra icons configured): the banner
  row still renders correctly with just attach-file (and PR comment, if
  configured).
- `onPrCommentClick` is not provided: no GitHub icon in either location,
  banner or footer.
- The restart action is in flight (`isResuming`): the restart button shows
  its loading state and stays disabled, matching today's Resume behavior.
- Locale coverage: the new/renamed labels exist in all 7 locales with no
  orphaned keys.

## Requirements

### Functional Requirements

- **FR-001**: While a session's latest coding-agent process is interrupted,
  the chat box MUST render its icon buttons (attach file, GitHub PR
  comments when available, `toolbarActions` items) inside the stopped-run
  row, to the left of the status label, and MUST NOT also render them in
  the footer.
- **FR-002**: While not interrupted, the chat box MUST render those same
  icon buttons in the footer exactly as before (unchanged position,
  handlers, and disabled logic).
- **FR-003**: The stopped-run row's button MUST be a start/restart action
  (not "Resume"), wired to the same callback and in-flight/disabled
  behavior the existing Resume button used.
- **FR-004**: The footer's status-driven action button (e.g. `Send`) MUST
  be unaffected by this change in every status, including interrupted.
- **FR-005**: All new or renamed labels MUST be translated keys present and
  consistent across all 7 locales (`en es fr ja ko zh-Hans zh-Hant`), and
  any fully-superseded key MUST be removed from all 7, not just `en`.

### Non-Goals

- Changing when a session counts as "interrupted"/stopped.
- Changing the editor, the `Send` button, or any other execution status's
  rendering.
- Introducing a new component outside `SessionChatBox`/`ChatBoxBase`.
- Changing backend behavior, `ExecutionProcessStatus`, or the resume prompt
  text.

## Measurable Acceptance Criteria

- **AC-001**: In a rendered-DOM test, a chat box with an interrupted-session
  notice renders its icon buttons exactly once, inside the banner row, and
  the footer's icon-button slot is empty.
- **AC-002**: In the same test, a restart button is present, a "Resume"
  labeled button is not, and clicking restart invokes the resume callback
  exactly once.
- **AC-003**: In a rendered-DOM test, a chat box without an interrupted
  notice renders its icon buttons in the footer (unchanged from current
  behavior) and shows no stopped-run row.
- **AC-004**: `node scripts/check-unused-i18n-keys.mjs` and a manual
  locale-key-set diff (see `IMPLEMENTATION_PLAN.md`) both pass clean across
  all 7 locales.

## Assumptions

- "Stopped" is an acceptable, discardable stand-in for the reference
  photo's status wording — the issue authorizes this once the real UI is
  identified (`PRIOR_KNOWLEDGE.md`).
- "Start/restart" maps to the existing resume-the-interrupted-run action;
  no new backend capability is required.

## Dependencies

- `ExecutionProcessStatus.interrupted` and the `hasInterruptedLatestProcess`
  computation in `SessionChatBoxContainer.tsx` already exist and are
  unchanged by this feature.
- `ToolbarIconButton`, `PrimaryButton`, and `ChatBoxBase`'s `banner`/
  `footerLeft`/`footerRight` slots already exist and are reused as-is.

## Validation

- Rendered-DOM (Vitest + Testing Library) coverage for both the interrupted
  and non-interrupted cases (icon-button location, restart button,
  callback).
- `pnpm run check`, `pnpm run lint`, `pnpm run format`.
- Manual check in the running dev app.
