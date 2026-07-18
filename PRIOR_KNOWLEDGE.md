# Prior Knowledge: Workspace Carousel View

Searched both project knowledge indexes (`wiki/INDEX.md` and
`docs/knowledge-base/INDEX.md`) for pages relevant to a horizontally-scrolling
multi-column workspace chat view sorted by "needs feedback". Two pages are
directly relevant, one is background.

## Directly relevant: "waiting for feedback" semantics already exist

`wiki/kanban-items-state-and-activity-grouping.md` (task
`vk/d4db-in-progress-acti`) defines the project's established
active-vs-waiting semantics for agent workspaces, used by the kanban
"In progress" Active/Waiting split:

- **Active** = linked non-archived workspace with `isRunning === true` **and**
  `hasPendingApproval !== true`. A run paused on tool approval counts as
  *waiting for feedback* even though a process is technically running — same
  semantics as the `IssueWorkspaceCard` hand icon.
- Anything without a live local workspace signal defaults to *waiting*.
- Helpers live in
  `packages/web-core/src/features/kanban/model/activityGrouping.ts` with unit
  tests alongside — reuse/extend rather than invent a parallel predicate.
- **Gotcha**: `workspacesByIssueId` is display-preference-gated (empty map when
  the `showWorkspaces` preference is off) — never reuse it for semantic
  logic; compute activity from the workspace context directly.
- Derived orderings must be applied in state-building code, not at render
  time, when they coexist with drag-and-drop index contracts. The carousel has
  no DnD, so a render-time `useMemo` sort is fine, but keep the lesson in mind
  if reordering interacts with any indexed structure.

Implication: the carousel's "needs feedback" tiering should be consistent with
this: pending-approval ⇒ needs feedback (leftmost) even if `isRunning` is
true; running-without-approval ⇒ does not need feedback.

## Directly relevant: horizontal multi-column scrolling gotchas

`wiki/mobile-kanban-scrolling.md` (task `vk/de6e-improve-column-s`) documents
the nested single-axis scroller architecture for a horizontally scrolling
column strip with vertically scrolling columns — exactly the carousel shape:

- Make each container scrollable on **exactly one axis**: outer strip
  `overflow-x-auto` (+ optionally `snap-x`) with `overflow-y-hidden`; each
  column's content `overflow-y-auto overflow-x-hidden overscroll-y-contain
  min-h-0`.
- **CSS spec gotcha**: `overflow-y: auto` with `overflow-x: visible` computes
  `overflow-x: auto` — a vertical scroller missing `overflow-x-hidden`
  silently becomes a competing horizontal scroller (iOS rubber-banding, stolen
  swipes). Any vertical scroller nested in the carousel must carry
  `overflow-x-hidden`.
- Even 1–2px of accidental horizontal overflow (negative-margin border
  tricks) turns a list into a real horizontal scroll container.
- Don't fix stolen swipes with `touch-action: pan-y` — it blocks horizontal
  gestures from reaching the outer strip entirely. Constrain `overflow-*`.
- Column headers belong inside the column but outside its vertical scroller,
  so they travel with the column horizontally.
- No touch engine exists in the task environment; mobile verification is
  manual (`mobile-testing.md` at repo root, phone-over-Tailscale).

## Background: how an agent turn "stops"

`wiki/agent-process-lifecycle.md` covers the backend one-turn-one-
`ExecutionProcess` model. For this task only the surface matters: the frontend
receives per-workspace `latest_process_status`
(`running|completed|failed|killed|interrupted`) and `is_running` via workspace
summaries/streams — no new backend work is needed to know an agent stopped.

## Not relevant (checked, skipped)

`docs/knowledge-base/` pages are executor/MCP/remote-integration focused;
`wiki/` pages on Electric fallback, task pipeline blocks, AppBar rail,
breadcrumbs, and repo-branch defaulting don't bear on this view. No page
covers the workspaces sidebar or chat-view provider stack — that knowledge
was gathered fresh by code exploration (see `SPEC.md`).
