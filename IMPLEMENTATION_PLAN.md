# Implementation Plan: Workspace Carousel View

Builds on `SPEC.md` and `PRIOR_KNOWLEDGE.md`. Frontend-only; no backend or
generated-type changes.

## Step 1 — Sort model + preference

1. `packages/web-core/src/shared/stores/useUiPreferencesStore.ts`:
   - Add `CarouselSort = 'needs_feedback' | 'updated_at' | 'created_at' | 'name'`.
   - Add `carouselSort` state (default `'needs_feedback'`) + `setCarouselSort`
     action, persisted to `localStorage` (follow the existing manual
     `mobileFontScale` read/write pattern since the store isn't `persist`-wrapped).
2. New pure module
   `packages/web-core/src/pages/workspaces/carousel/carouselSort.ts`:
   - `needsFeedback(ws)` = `ws.hasPendingApproval || (ws.hasUnseenActivity && !ws.isRunning)`
     (consistent with `WorkspacesSidebar` needs-attention and the kanban
     activity-split semantics: pending approval ⇒ needs feedback even while running).
   - `carouselTier(ws)`: 0 needs-feedback, 1 failed/killed latest process,
     2 idle, 3 running.
   - `sortForCarousel(workspaces, sort)`: tiered sort for `needs_feedback`
     (ties: latest activity desc), plain comparators for the others.
   - Vitest unit tests alongside (`carouselSort.test.ts`).

## Step 2 — Per-column data stack (no provider refactor)

3. **Verified**: nothing under `features/workspace-chat/` consumes
   `WorkspaceContext`, and `WorkspacesMainContainer` is fully prop-driven.
   Each column therefore mounts only
   `ExecutionProcessesProvider(sessionId)` around a `WorkspacesMainContainer`
   fed by `useWorkspaceRecord(workspaceId)` /
   `useWorkspaceSessions(workspaceId)` / `useWorkspaceRepo(workspaceId)`.
   `WorkspaceProvider` is **not** mounted per column — that avoids its
   on-mount `markSeen` (which would erase the unseen-activity signal the
   default sort needs), its global `useWorkspaceDiffStore` writes, and N
   diff/GitHub websockets. Columns mark a workspace seen only when the user
   focuses that column's chat input. Diff stats in columns read a cleared
   store and show zeros — accepted for v1 (see
   `homelab/specs/vk/f9c1-vk-workspace-car/research.md`).

## Step 3 — Column component

5. `packages/web-core/src/pages/workspaces/carousel/WorkspaceCarouselColumn.tsx`:
   - Props: `workspace` (sidebar/UI shape), mounted flag (for windowing).
   - Header (outside the vertical scroller): name, project/branch, status
     badge (needs feedback / failed / running / idle), open-in-full-view link
     (navigates to the existing `/workspaces/$workspaceId` route).
   - Body: `useWorkspaceRecord`/`useWorkspaceSessions`/`useWorkspaceRepo`
     with the column's `workspaceId`, then
     `ExecutionProcessesProvider(sessionId=latest)` →
     `WorkspacesMainContainer` (it instantiates its own per-chat providers).
     No `ReviewProvider`/`ChangesViewProvider` needed (`useReviewOptional`
     is null-safe; no changes panel in columns).
   - Scroll containment per prior knowledge: column content
     `overflow-y-auto overflow-x-hidden overscroll-y-contain min-h-0`.

## Step 4 — Carousel page

6. `packages/web-core/src/pages/workspaces/WorkspacesCarousel.tsx`:
   - Data from `useWorkspaceContext()` (`activeWorkspaces`) — the `_app`
     layout already provides the list context.
   - Toolbar: sort `Select` (4 options), "N need feedback" count, button back
     to the standard workspaces view.
   - Sorted list via `sortForCarousel` in a `useMemo`; **debounce** order
     changes ~1s and freeze order while any column's input has focus
     (track focus via a per-column callback or `focusin`/`focusout` on the strip).
   - Strip: `flex-1 flex overflow-x-auto overflow-y-hidden`; columns keyed by
     `workspace.id`, fixed width (`w-[420px] shrink-0`).
   - Mount windowing: render the full chat only for columns within the render
     window (index-based window around the scroll position or
     IntersectionObserver, cap ~8 mounted chats); other columns show a
     placeholder with header + status.
   - Empty state when no active workspaces.

## Step 5 — Routing + entry points

7. `packages/web-core/src/shared/lib/routes/appNavigation.ts`: add
   `{ kind: 'workspaces-carousel' }` to `AppDestination`.
8. `packages/local-web/src/app/navigation/AppNavigation.ts`: map path
   `/workspaces/carousel` ↔ destination; add `goToWorkspacesCarousel()`
   builder.
9. New route `packages/local-web/src/routes/_app.workspaces_.carousel.tsx`
   rendering `WorkspacesCarousel` (routeTree.gen.ts regenerates via the Vite
   plugin — do not hand-edit).
10. Entry point: add a carousel toggle button in the workspaces sidebar
    header (next to the existing controls in `WorkspacesSidebar`/its
    container) and a "standard view" button in the carousel toolbar.

## Step 6 — Verification

11. `pnpm run check`, `pnpm run lint`, unit tests (`vitest` for
    `carouselSort`), `pnpm run format`.
12. Runtime verification: run the dev app, open `/workspaces/carousel`,
    confirm columns render, chat follow-up works, and ordering reacts to
    status changes.

## Order & parallelism

Step 1 and Step 2 are independent; Steps 3–4 depend on both; Step 5 depends
on 4; Step 6 last.

## Risk notes

- The `WorkspacesMainContainer` reuse (Step 3) is the highest-uncertainty
  item: it may pull in panel/layout state tied to the single-workspace page.
  Fallback: render `ConversationList` + `SessionChatBoxContainer` directly
  with the minimal leaf providers, accepting reduced feature surface
  (no plan-mode banner etc.) in the carousel.
- Any vertical scroller added inside columns must carry `overflow-x-hidden`
  (CSS overflow promotion gotcha) or it will fight the strip for horizontal
  pan gestures.
