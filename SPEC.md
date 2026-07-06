# Spec: Split the "In progress" kanban column by activity

Task: `vk/d4db-in-progress-acti` — "the 'in progress' status can be both
actively working or waiting for feedback. Split issues in this category into
their active group."

## Problem

On the project kanban board, an issue sits in the **In progress** column both
while a coding agent is actively working on one of its workspaces *and* after
the agent has stopped and is waiting on the user (review the output, answer a
question, approve a tool call, send a follow-up prompt). These are very
different states for the user — one needs no attention, the other is blocked
on them — but the column renders them as one undifferentiated list. Users must
open each card (or squint at the tiny per-workspace status icons) to find out
which issues are actually waiting on them.

## Goal

Inside the **In progress** column of the board view, partition the issues into
two visually separated groups, each with a small section header:

1. **Active** — at least one linked workspace has an agent actively running
   (and not blocked on a tool-approval prompt).
2. **Waiting for feedback** — everything else: the latest agent run finished
   (completed/failed/killed), a run is paused on a pending tool approval, or
   the issue has no live workspace signal at all.

Cards move between groups automatically as live workspace state changes (agent
starts → Active; agent finishes or asks for approval → Waiting for feedback).

## Non-goals

- **No backend / schema / API changes.** The activity signal already reaches
  the frontend (workspace WS stream + workspace summaries); this is a pure
  presentation change in `web-core`.
- **No change to the issue's `status_id`** — grouping is derived, ephemeral UI
  state. Nothing is persisted.
- **No new columns.** "In review" already exists as a separate status for the
  human-review stage of the workflow; this feature is about live agent
  activity *within* "In progress", not about adding workflow stages.
- **List view (`IssueListView`) unchanged.** The feature targets the board
  ("kanban" and "slim") views where the In progress column is rendered.
- No user setting to toggle the grouping (it only appears when it has
  something to say — see Behavior).

## Background: where the signals live

- The board renders dynamic per-project `ProjectStatus` columns
  (`packages/web-core/src/features/kanban/ui/KanbanContainer.tsx`). "In
  progress" is a seeded status row (`crates/remote/src/db/project_statuses.rs`
  `DEFAULT_STATUSES`), not an enum variant. The backend itself identifies it
  by name (`ProjectStatusRepository::find_by_name(pool, project_id,
  "In progress")` in `crates/remote/src/db/issues.rs::
  sync_issue_from_workspace_created`), so name matching is the established
  pattern.
- Live per-workspace agent state arrives via `useWorkspaceContext().
  activeWorkspaces` (`SidebarWorkspace` from
  `packages/web-core/src/shared/hooks/useWorkspaces.ts`): `isRunning` (WS
  stream), `hasPendingApproval`, `hasUnseenActivity`, `latestProcessStatus`
  (summaries API).
- `KanbanContainer` already joins issues to their live workspaces in the
  `workspacesByIssueId` memo, but that memo is gated behind the
  `showWorkspaces` display preference; the grouping signal must not be.

## Functional requirements

### FR1 — Which columns are grouped

A status column gets the activity grouping when its name matches the seeded
in-progress status: `status.name.trim().toLowerCase() === 'in progress'`.
Case-insensitive to be tolerant of cosmetic renames, consistent in spirit with
the backend's name-based lookup. Renamed/custom statuses keep today's
behavior.

### FR2 — Group assignment

For each issue in a grouped column:

- **Active** ⇔ at least one linked, non-archived workspace that resolves to a
  live local workspace has `isRunning === true` **and**
  `hasPendingApproval !== true`.
- **Waiting for feedback** ⇔ every other issue in the column, including:
  - runs paused on a pending tool approval (`isRunning && hasPendingApproval`
    — the agent is literally waiting for the user),
  - latest process completed / failed / killed / interrupted,
  - issues with no linked workspace or no live local workspace signal
    (e.g. workspaces owned by another machine): nothing is running for the
    user, so they are "waiting" by default.

This mirrors the per-workspace status-icon semantics in
`packages/ui/src/components/IssueWorkspaceCard.tsx` (spinner = running,
hand = pending approval, dot = unseen activity).

The group signal is computed from `activeWorkspaces` + issue→workspace links
independently of the `showWorkspaces` preference.

### FR3 — Ordering & sorting

- Within the grouped column, **Active** issues render first, then **Waiting
  for feedback**. (The column reads top-down as "what is happening right now",
  and the groups make the waiting set explicit immediately below.)
- The partition is **stable**: within each group the existing sort
  (`sortField`/`sortDirection`, including manual `sort_order`) is preserved
  unchanged.
- The partition is applied where column membership is computed (the `items`
  rebuild effect in `KanbanContainer`), so the rendered order and the
  `items[statusId]` array stay identical — this is what drag-and-drop indexes
  and `calculateSortOrder` are based on.

### FR4 — Group headers

- A small, non-draggable section header row is rendered above each group:
  "Active" and "Waiting for feedback", each with the group's card count.
- Headers are only rendered when the column contains **both** groups. A column
  that is all-active or all-waiting renders exactly as today (no header
  noise).
- Header labels are i18n'd (new keys under `kanban.` in
  `packages/web-core/src/i18n/locales/*/common.json` for all locales: en, es,
  fr, ja, ko, zh-Hans, zh-Hant).
- Headers render in both `kanban` and `slim` board modes, desktop and mobile.
- Headers are inert for drag-and-drop: they are plain elements between
  `Draggable` cards; card `index` props remain the flat position in
  `items[statusId]` so `@hello-pangea/dnd` indexes stay correct.

### FR5 — Drag-and-drop interaction

- Cross-column and within-column DnD keep today's semantics untouched
  (`handleDragEnd` unchanged).
- Dropping a card into the "wrong" group is allowed; since group membership is
  derived from live state, the next `items` rebuild snaps it back into its
  correct group (keeping its manual order within that group). No attempt is
  made to block drops per group.

### FR6 — Liveness

Group membership re-derives whenever workspace state changes (the memoized
activity map is an input to the `items` rebuild effect), so a card jumps from
Active to Waiting for feedback the moment its agent stops, without a refresh.

## UI

Header row (per group), inside `KanbanCards` above the group's first card:

```
ACTIVE · 2                 ← text-xs uppercase, muted (text-low), py-half px-base
─ cards… ─
WAITING FOR FEEDBACK · 3
─ cards… ─
```

Minimal chrome — no icons, no background; it must read as a subdivision of the
column, not a new column header. Exact classes follow the design system
(`packages/local-web/AGENTS.md`).

## Implementation sketch

All in `packages/web-core` (+ i18n files):

1. New pure helper module `packages/web-core/src/features/kanban/model/
   activityGrouping.ts`:
   - `isInProgressStatus(name: string): boolean`
   - `isWorkspaceActive(ws): boolean` (`isRunning && !hasPendingApproval`)
   - `partitionByActivity(issueIds, activeIssueIds): string[]` — stable
     active-first ordering.
   - `buildActivityGroups(issueIds, activeIssueIds)` — render-side segments
     (`active` / `waiting` id arrays) so the component can place headers.
2. `KanbanContainer.tsx`:
   - Compute `activeIssueIds: Set<string>` from `issues` ×
     `getWorkspacesForIssue` × `localWorkspacesById` (not gated by
     `showWorkspaces`).
   - In the `items` rebuild effect: after sorting a status's issues, if
     `isInProgressStatus(status.name)`, stable-partition active-first. Add the
     activity signal to the effect deps.
   - In the board render loop: for grouped columns compute the two segments
     from `items[status.id]` + `activeIssueIds`; render header rows (only if
     both segments non-empty) and keep the flat card `index`.
3. i18n: add `kanban.activityGroups.active` / `.waitingForFeedback` to all
   seven locale `common.json` files.
4. Vitest unit tests for the helper module (partition stability, header
   eligibility, active predicate incl. pending-approval case), colocated as
   `activityGrouping.test.ts`.

## Acceptance criteria

- In progress column with 2 running-agent issues and 3 idle issues shows
  "Active · 2" above the running ones and "Waiting for feedback · 3" above the
  rest; other columns unchanged.
- A workspace whose agent is paused on tool approval appears under Waiting for
  feedback.
- All-active or all-waiting In progress column shows no headers.
- Grouping appears even when the "show workspaces" display preference is off.
- Manual sort order is preserved within each group; DnD between columns still
  works; within-column reorder still only in manual-sort mode.
- A card moves between groups live when its agent starts/stops.
- `pnpm run check`, `pnpm run lint`, and web-core vitest pass; no Rust or
  generated-type changes.
