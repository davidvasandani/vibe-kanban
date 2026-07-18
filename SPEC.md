# Spec: Workspace Carousel View

## Summary

An alternative workspace view for the local web app that renders multiple workspace chats side-by-side as **vertical columns scrolling horizontally** (a "carousel"), with a sort control. The default sort is **"Needs feedback"**: workspaces whose agent has stopped, is awaiting tool approval, or has produced unseen output pop to the **left** of the strip, so the user can keep every agent moving by always working the leftmost columns.

Motivation: when running many agents concurrently, the single-chat-at-a-time layout hides which agents are blocked. The carousel is a "triage cockpit" — each column is a live, fully interactive chat (conversation history + follow-up input), and the ordering surfaces whichever agent needs a human next.

## Goals

- New route `/workspaces/carousel` in `packages/local-web` rendering active workspaces as columns.
- Each column shows: workspace header (name, status), the session conversation, and the follow-up chat box — fully functional (send follow-ups, approve tools).
- Horizontal scrolling across columns; each column scrolls vertically independently.
- Sort selector with at least: **Needs feedback** (default), **Recently updated**, **Recently created**, **Name**. Sort choice persisted.
- "Needs feedback first" ordering updates live as workspace statuses change (existing streams already push updates).
- Entry point from the existing workspaces UI (toolbar button) and a way back to the standard view.

## Non-goals

- No changes to the remote (`packages/remote-web`) frontend.
- No backend/API changes: all required status fields (`has_pending_approval`, `has_unseen_turns`, `latest_process_status`, `is_running`) already exist on workspace summaries/streams.
- No drag-to-reorder, no per-column diff/changes panel, no multi-session tabs per column (latest/selected session only).
- Mobile layout optimization (usable via horizontal scroll is enough).

## Design

### Data

- Workspace list + statuses come from the existing `useWorkspaces` hook (`packages/web-core/src/shared/hooks/useWorkspaces.ts`): WS JSON-patch streams `/api/workspaces/streams/ws?archived=false` plus the `/api/workspaces/summaries` poll. Fields used per workspace: `isRunning`, `hasPendingApproval`, `hasUnseenActivity`, `latestProcessStatus`, `latestProcessCompletedAt`, `updatedAt`, `createdAt`, `isPinned`.
- "Needs feedback" predicate reuses the sidebar's existing needs-attention logic (`packages/ui/src/components/WorkspacesSidebar.tsx`):
  `needsFeedback(ws) = ws.hasPendingApproval || (ws.hasUnseenActivity && !ws.isRunning)`
  Tier ordering for the default sort:
  1. Needs feedback (approval pending first, then stopped-with-unseen-output), most recent activity first within tier
  2. Failed/errored latest process (`failed`, `killed`) — agent stopped abnormally
  3. Idle (not running, nothing unseen)
  4. Running (agent is fine on its own — rightmost)
  Pinned workspaces do **not** override the needs-feedback tiering in this view (the whole point is triage order); ties break by `latestProcessCompletedAt`/`updatedAt` descending.

### Per-column provider stack

The chat components require per-workspace context. Each column mounts:

`WorkspaceProvider(workspaceId)` → `ExecutionProcessesProvider(sessionId)` → `ReviewProvider(workspaceId)` → `ChangesViewProvider` → chat UI (`WorkspacesMainContainer` internals or a trimmed column variant rendering `ConversationList` + `SessionChatBoxContainer` with their leaf providers `ApprovalFeedbackProvider`/`EntriesProvider`/`MessageEditProvider`).

`WorkspaceProvider` currently reads `workspaceId` from route params; it gains an optional `workspaceId` prop (falling back to params) so it can be instantiated N times. Its writes to the global `useWorkspaceDiffStore` must be keyed or disabled for carousel instances so N columns don't clobber one store.

Each column uses the workspace's **latest session** (same default as the main view).

### Performance

Mounting dozens of live chat streams is expensive. The carousel windows the mounted chats: only columns near the viewport mount the full chat (conversation + log streams); off-screen columns render a lightweight placeholder header. A hard cap (render window of ~8 mounted chats) keeps WS connection counts sane.

### Layout

- Full-width strip: `flex overflow-x-auto` container, columns `w-[420px] shrink-0 h-full flex flex-col border-r` (Kanban container pattern, `packages/web-core/src/features/kanban/ui/KanbanContainer.tsx`).
- Column header: workspace name, project/branch, status badge (needs-feedback / running / failed / idle), click-through to the full single-workspace view.
- Toolbar above the strip: sort selector, count summary (e.g. "3 need feedback"), link back to standard view.
- Columns keep stable React keys (`workspace.id`) so column state (scroll position, draft input) survives re-ordering. Re-sorting applies on status change with a short debounce (~1s) to avoid thrash while a stream flaps; the focused column never moves while its chat box has focus.

### Routing & preferences

- Route file `packages/local-web/src/routes/_app.workspaces_.carousel.tsx` → new page `packages/web-core/src/pages/workspaces/WorkspacesCarousel.tsx`; inherits the `_app` provider stack.
- Add destination kind to `AppDestination` (`packages/web-core/src/shared/lib/routes/appNavigation.ts`) + mapping/builder in `packages/local-web/src/app/navigation/AppNavigation.ts`.
- Sort choice stored in `useUiPreferencesStore` (new `carouselSort` field) with localStorage persistence, defaulting to `needs_feedback`.

## Acceptance criteria

1. Navigating to `/workspaces/carousel` shows active (non-archived) workspaces as horizontal-scrolling vertical columns, each with a working conversation view and follow-up input.
2. Default sort is "needs feedback": a workspace with a pending approval or with unseen agent output while not running appears left of running workspaces; when an agent stops/asks a question, its column moves left without a page reload.
3. Sort can be switched (needs feedback / updated / created / name) and the choice survives reload.
4. Sending a follow-up from any column works and that column's status flips to running (moving it rightward under the default sort after debounce).
5. Column identity is stable across re-sorts — draft text in a column's chat box survives a re-order.
6. `pnpm run check` and `pnpm run lint` pass; no hand edits to generated files.

## Risks

- Provider refactor (`WorkspaceProvider` prop-ification, diff-store keying) touches the existing single-workspace view — mitigated by the prop defaulting to route params and behavior-preserving fallbacks.
- Resource usage with many live columns — mitigated by mount windowing.
- Re-sort motion could be disorienting mid-typing — mitigated by debounce, stable keys, and freezing order while a column's input has focus.
