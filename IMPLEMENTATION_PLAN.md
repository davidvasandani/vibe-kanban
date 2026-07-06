# Implementation Plan: Split the "In progress" column by activity

See `SPEC.md` for the full design and `../PRIOR_KNOWLEDGE.md` for mobile
card-list constraints. All changes are frontend (`packages/web-core`); no
backend, schema, or generated-type changes.

## Step 1 — Pure grouping helpers + unit tests

**New file:** `packages/web-core/src/features/kanban/model/activityGrouping.ts`

- `isInProgressStatus(name: string): boolean` — `trim().toLowerCase() === 'in progress'`.
- `isWorkspaceActive(ws: { isRunning?: boolean; hasPendingApproval?: boolean }): boolean`
  — `isRunning === true && hasPendingApproval !== true` (a run paused on tool
  approval is *waiting*, matching `IssueWorkspaceCard`'s hand-icon semantics).
- `partitionByActivity(issueIds: string[], activeIssueIds: ReadonlySet<string>): string[]`
  — stable partition, active ids first, original order preserved within each
  group. Returns the input array unchanged (same reference not required) when
  no reordering is needed.
- `buildActivityGroups(issueIds: string[], activeIssueIds: ReadonlySet<string>)`
  → `{ active: string[]; waiting: string[]; showHeaders: boolean }` where
  `showHeaders = active.length > 0 && waiting.length > 0`.

**New file:** `activityGrouping.test.ts` (Vitest, colocated) covering:
- name matching (case/whitespace variants, non-matches like "In review");
- active predicate (running, running+pendingApproval, not running, undefined
  flags);
- partition stability and no-op cases (all active / all waiting / empty);
- `showHeaders` only when both groups non-empty.

## Step 2 — Activity signal in `KanbanContainer`

**Edit:** `packages/web-core/src/features/kanban/ui/KanbanContainer.tsx`

- New memo `activeIssueIds: Set<string>` computed from `issues`,
  `getWorkspacesForIssue`, `localWorkspacesById` (the existing
  `SidebarWorkspace` map): an issue id is in the set when any linked,
  non-archived workspace resolves to a local workspace with
  `isWorkspaceActive(localWorkspace)`. **Not** gated by `showWorkspaces`
  (unlike `workspacesByIssueId`) — grouping must work with workspace cards
  hidden. Reuses the same link-resolution shape as `workspacesByIssueId`
  (`!ws.archived && ws.local_workspace_id && localWorkspacesById.has(...)`).
- In the `items` rebuild effect (currently lines ~478–525): after the
  per-status sort, apply
  `if (isInProgressStatus(status.name)) statusIssueIds = partitionByActivity(...)`.
  Add `activeIssueIds` to the effect dependency array so group membership
  re-derives live when agents start/stop. The `isSyncingRef` drag-sync skip
  stays as is.

## Step 3 — Group header rendering

**Edit:** same file, board render loop (currently lines ~986–1128):

- For each column, compute `const groups = isInProgressStatus(status.name)
  ? buildActivityGroups(issueIds, activeIssueIds) : null` (in-render, cheap).
- When `groups?.showHeaders`, render a header row before the first card of
  each group inside `<KanbanCards>`; determine boundaries while mapping the
  flat `issueIds` (header before index 0, and before the first id whose group
  differs from the previous id's group). Cards keep their **flat** `index`
  prop so `@hello-pangea/dnd` draggable indexes keep matching
  `items[status.id]` and `handleDragEnd`/`calculateSortOrder` stay untouched.
- Header row markup (per PRIOR_KNOWLEDGE constraints: plain block element, no
  negative margins, no scroll container):
  `<div className="flex items-center gap-half px-base pt-base pb-half text-xs uppercase tracking-wide text-low">`
  with label + count. Use `t('kanban.activityGroups.active')` /
  `t('kanban.activityGroups.waitingForFeedback')`.
- Applies to both `kanban` and `slim` modes automatically (same loop). List
  view untouched.

## Step 4 — i18n

**Edit:** `packages/web-core/src/i18n/locales/{en,es,fr,ja,ko,zh-Hans,zh-Hant}/common.json`

Add under the existing `"kanban"` object:

- en: `activityGroups.active` = "Active", `activityGroups.waitingForFeedback`
  = "Waiting for feedback"
- es: "Activo" / "Esperando comentarios"
- fr: "En cours d'exécution" → keep short: "Actif" / "En attente de retour"
- ja: "実行中" / "フィードバック待ち"
- ko: "실행 중" / "피드백 대기 중"
- zh-Hans: "进行中" / "等待反馈"
- zh-Hant: "進行中" / "等待回饋"

(Mirror the nesting style already used in each file's `kanban` section.)

## Step 5 — Verification

1. `pnpm -F @vibe/web-core exec vitest run src/features/kanban/model/activityGrouping.test.ts`
   (or the package's test script) — new unit tests green.
2. `pnpm run check` — TS across web + Rust workspaces.
3. `pnpm run lint` — ESLint + clippy.
4. `pnpm run format` before finishing (repo requirement).
5. Manual sanity if feasible (`pnpm run dev`): seed a project, start an agent
   on one In progress issue, confirm headers/split appear, approval-pending
   run lands in Waiting, other columns unaffected, no headers when
   single-group. Mobile touch checks are manual-only per
   `mobile-testing.md` — note in PR.

## Risks / edge cases to watch

- **Effect deps:** `activeIssueIds` must be memoized with stable identity
  (rebuild only when inputs change) to avoid rebuilding `items` every render;
  derive from `issues` + `getWorkspacesForIssue` + `localWorkspacesById`.
- **DnD index integrity:** headers must not be `Draggable`s; card `index`
  stays the flat array position (dnd requires indexes to match render order
  of draggables within a droppable — inert elements between them are fine).
- **Snap-back UX:** dropping a card across group headers reorders the flat
  array; the next rebuild snaps it to its derived group. Accepted per spec
  FR5.
- **Renamed statuses** don't group (FR1) — intentional, matches backend
  name-based "In progress" behavior.

## Pipeline stages remaining after implementation

- Codex review of the diff (iterate until clean).
- Wiki: add a page on kanban activity grouping / issue-workspace activity
  signals; update `wiki/INDEX.md`; tag with `vk/d4db-in-progress-acti`.
- Open PR against the base branch.
