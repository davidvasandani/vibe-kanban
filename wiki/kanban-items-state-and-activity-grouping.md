# Kanban `items` state, DnD contract, and the In progress activity split

How `KanbanContainer` (`packages/web-core/src/features/kanban/ui/KanbanContainer.tsx`)
turns issues into rendered columns, the invariants anyone touching that path
must preserve, and how the "In progress" activity grouping builds on them.

## The `items` state is a contract, not a cache

`items: Record<statusId, issueId[]>` is rebuilt by an effect from
`filteredIssues` + per-column sort. Three things depend on the *exact array
order*:

1. **Board render order** — cards are mapped straight off `items[status.id]`.
2. **`@hello-pangea/dnd` indexes** — each `KanbanCard`'s `index` prop must be
   its flat position in that array; `handleDragEnd` splices
   `prev[droppableId]` at `source.index`/`destination.index`. If render order
   and array order diverge, drags move the wrong card.
3. **Persisted `sort_order`** — after a drop, every issue in the affected
   column(s) is written as `1000 * columnIndex + issueIndex` from the array.

Corollaries:

- Any derived reordering (like the activity partition) must happen **inside
  the rebuild effect**, never at render time, so (1) and (2) stay in sync.
- Inert (non-`Draggable`) elements *between* cards are fine — dnd only
  requires draggable indexes to be sequential in render order. This is how
  the activity group headers are inserted without touching `handleDragEnd`.
- `items` feeds **both** the board and `IssueListView`. A transform meant for
  the board only must gate on `isBoardView` (kanban/slim), or it leaks into
  list view — and worse, list-view drags then persist `sort_order` computed
  from the transformed order.

## Gotcha: the `isSyncingRef` drag-sync window swallows rebuilds

After a drop, `isSyncingRef.current = true` for ~500ms (while
`bulkUpdateIssues` + Electric sync settle) and the rebuild effect returns
early. Dep changes that arrive **during** the window are lost — the effect
ran, skipped, and will not rerun when the flag clears. Before the activity
grouping this was harmless (the dropped order *was* the desired order); with
any derived ordering it leaves stale state. Fix in place: an
`itemsRebuildTick` state is bumped in the same `setTimeout` that clears the
flag (in `.finally`, so error paths too), and the tick is an effect dep — one
guaranteed rebuild per drag. Reuse the tick if you add other derived
transforms; don't add a second mechanism.

## In progress activity split (derived, ephemeral)

The "In progress" column partitions into **Active** (agent running) and
**Waiting for feedback** groups; helpers live in
`packages/web-core/src/features/kanban/model/activityGrouping.ts` (unit
tests alongside):

- **Group signal**: an issue is *active* iff some linked, non-archived
  workspace resolves through `local_workspace_id` into
  `useWorkspaceContext().activeWorkspaces` with `isRunning === true` and
  `hasPendingApproval !== true`. A run paused on tool approval is *waiting*
  — same semantics as `IssueWorkspaceCard`'s hand icon. Issues with no live
  local workspace signal (none linked, archived, other machine's workspace)
  default to *waiting*.
- **Nothing is persisted**: grouping never touches `status_id`/`sort_order`;
  it is a stable active-first partition applied after the user's sort.
  Dropping a card into the "wrong" group is allowed and snaps back on the
  next rebuild (guaranteed by the tick above).
- **Headers render only when both groups are non-empty**, so quiet boards
  look exactly as before.
- i18n keys: `kanban.activityGroups.{active,waitingForFeedback}` in all
  seven locales.

## Gotcha: `workspacesByIssueId` is preference-gated; the activity signal must not be

The existing `workspacesByIssueId` memo returns an **empty map** when the
`showWorkspaces` display preference is off — it drives the workspace
mini-cards, not semantics. Anything that needs issue↔agent activity
regardless of display settings must compute from
`getWorkspacesForIssue` × `localWorkspacesById` directly (see the separate
`activeIssueIds` memo). Don't "reuse" `workspacesByIssueId` for logic.

## Convention: "In progress" is identified by name

There is no status kind/category field. The backend moves issues to
In progress by literal name lookup
(`ProjectStatusRepository::find_by_name(pool, project_id, "In progress")` in
`crates/remote/src/db/issues.rs::sync_issue_from_workspace_created`), and the
frontend activity grouping matches `name.trim().toLowerCase() ===
'in progress'`. A renamed column silently opts out of both behaviors — this
is accepted, established behavior; if a semantic status category is ever
added, both sites should migrate together.

## Contributed by

- vk/d4db-in-progress-acti
