# SPEC: Fix mobile issue list UI

Task: `vk/2b39-fix-mobile-ui`. Feature spec, plan, tasks and mockups:
`specs/vk/2b39-fix-mobile-ui/`.

## Problem

On a phone (iPhone, ~390px CSS width) the project **list view** (All / Active /
Backlog tabs) is unreadable:

- Rows with tags + relationship badges overflow the viewport. The badges paint
  **on top of** the issue id (`feature 41↗ SWE-142` — "SWE-141" hidden under the
  tag) and the age column is clipped (`52c` instead of `52d`).
- Rows with only a tag squeeze the title to an ellipsis (`C1 ...`, `Auto...`)
  or hide it entirely — the title, the most important field, is the first thing
  dropped.
- "Clear filters" renders as a full text button and pushes the filter row wide.

### Root cause

`IssueListRow` (`packages/ui/src/components/IssueListRow.tsx`) is a single
`justify-between` flex row. The right cluster (≤2 tags, ≤2 relationships, +N,
assignee, age) is `shrink-0`; the left cluster is `flex-1 min-w-0` but its
handle/id/dot children are `shrink-0`. When the right cluster is wider than
the viewport the left cluster shrinks to 0 and its non-shrinking children
overflow underneath the right cluster. Nothing clips the row horizontally, and
the list scroller only sets `overflow-y-auto` (which computes `overflow-x` to
`auto` — see wiki `mobile-kanban-scrolling.md`, Gotcha 1).

## Goal

On mobile the list view shows, for every row, without overlap or clipping:
id, status, **readable title**, age; and secondary metadata (tags,
relationships, assignees) on a second line that may truncate but never
overlaps. Desktop is unchanged.

## Requirements

- R1 Mobile list rows use a two-line layout:
  line 1 = drag handle/checkbox, priority, id, status dot, title (flex,
  truncates), age (right-aligned, never clipped);
  line 2 (only when the issue has tags, relationships or assignees) = tags,
  relationships (+N overflow count), assignee aligned right. Line 2 clips
  (`overflow-hidden`) instead of overflowing.
- R2 Desktop (`isMobile` false/undefined) row markup and classes are unchanged.
- R3 `isMobile` is threaded `KanbanContainer → IssueListView →
  IssueListSection → IssueListRow` as an optional prop (the `ui` package
  convention — see `KanbanFilterBar`, `KanbanCardContent`).
- R4 Mobile list scroller uses `px-base` (matches the header padding) and
  `overflow-x-hidden` so it is never a horizontal scroller.
- R5 Mobile filter bar renders "Clear filters" as an icon-only button
  (accessible label kept) so Team/Personal, search, filter, clear and + stay
  on one line at 390px.
- R6 Mockups of the alternatives considered are checked in at
  `specs/vk/2b39-fix-mobile-ui/mockups/` (HTML + rendered PNG).
- R7 Drag-and-drop, click-to-open, multi-select checkbox keep working.

## Non-goals

- Board (kanban column) view, navbar, issue panel.
- Changing which tags/relationships are shown or their limits.

## Acceptance

- `pnpm run check` and `pnpm run lint` pass; `pnpm run format` clean.
- Rendering the mockup/real row at 390px shows no overlap and the full age.
