# Workspace carousel view & rendering N workspace chats at once

How the carousel view (`/workspaces/carousel`,
`packages/web-core/src/pages/workspaces/WorkspacesCarousel.tsx` + the
`carousel/` directory beside it) renders many live workspace chats
side-by-side, and the reusable lessons about the chat component stack that
made it possible. Complements [mobile-kanban-scrolling.md] for the
horizontal-strip CSS rules.

## The chat pane is prop-driven — no `WorkspaceProvider` needed per instance

Nothing under `features/workspace-chat/` consumes `WorkspaceContext`.
`WorkspacesMainContainer` is fully prop-driven (`selectedWorkspace`,
`selectedSession`, `sessions`, `repos`, callbacks) and instantiates its own
per-chat providers (`ApprovalFeedback`/`Entries`/`MessageEdit`/`RetryUi`).
`useReviewOptional` is null-safe, so `ReviewProvider`/`ChangesViewProvider`
are optional. A fully working chat instance is just:

```
<ExecutionProcessesProvider sessionId={latestSessionId}>
  <WorkspacesMainContainer selectedWorkspace={record} … />
</ExecutionProcessesProvider>
```

fed by `useWorkspaceRecord` / `useWorkspaceSessions` / `useWorkspaceRepo`
with an explicit `workspaceId` (all route-independent;
`useWorkspaceSessions` auto-selects the most recently used session).

**Do not mount `WorkspaceProvider` per instance.** Three reasons, all
verified: (1) its mount effect calls `workspacesApi.markSeen`, which erases
the `hasUnseenActivity` signal for every visible workspace; (2) its diff
effect writes the global `useWorkspaceDiffStore` singleton — N writers
clobber each other; (3) it opens diff + GitHub-comment websockets per
instance. The `_app` layout's route-level `WorkspaceProvider` still supplies
the workspace *list* (`useWorkspaceContext().activeWorkspaces`) on any
param-less route, with its per-workspace effects inert.

Two small props were added to `WorkspacesMainContainer` for out-of-route
mounting: `diffStatsOverride` (chat-box stats from workspace summaries
instead of the diff store — otherwise columns show a false "0 files
changed") and `hideContextBar` (the context bar's actions resolve through
the route-level `useActionVisibilityContext`, which is empty outside the
single-workspace route, so it must not render there).

## Gotcha: chat editors autofocus — focus is not a user signal

The session chat editor autofocuses when it mounts. Any logic keyed on
focus entering a chat ("the user is looking at this") fires spuriously on
mount. The carousel originally marked workspaces seen on `focusCapture` and
the page instantly cleared its own needs-feedback tier for every rendered
column (caught in runtime verification, not review). Use real interaction —
`onPointerDownCapture` / `onKeyDownCapture` — as the "user engaged with this
chat" signal instead of focus. Blur is similarly unreliable for release
(focus may never have been where you think); the carousel releases its
order-freeze when a pointerdown lands outside any column.

## Debounced re-sort that must not starve

The carousel applies live re-sorting through a ~1s debounce, frozen while
the user is engaged with a column. Two traps:

- Workspace summaries repoll every 15s and the WS stream patches often, so
  the sort input changes identity frequently with *equal content*. An
  effect that clears/re-arms its debounce timer in its cleanup resets the
  countdown on every unrelated update and can starve the re-sort forever.
  Compare content (`arraysEqual`) before touching the timer, and let an
  armed timer survive unrelated effect re-runs (it reads the latest target
  from a ref when it fires).
- Freeze bookkeeping in refs must be pruned when a workspace disappears
  from the list (archived while engaged), or the order stays frozen
  permanently — a column that unmounts never fires blur.

Column identity: key columns by `workspace.id` and reorder in place —
React moves the DOM nodes and editor drafts/scroll positions survive.
Mount-windowing (live chat only near the viewport, placeholder elsewhere)
bounds websocket count; keep recently-interacted columns live regardless of
window so their drafts aren't dropped by unmounting.

## Needs-feedback semantics (extends the kanban Active/Waiting split)

`needsFeedback(ws) = hasPendingApproval || (hasUnseenActivity && !isRunning)`
— same predicate as the sidebar Needs Attention group. Triage tiers for the
default sort: needs-feedback, stopped-abnormally, idle, running. **Stopped
abnormally includes `interrupted`**, not just `failed`/`killed` — the chat
surfaces a resume action for interrupted runs, so they need a human too
(first-round Codex review finding). Pure sort logic lives in
`packages/web-core/src/pages/workspaces/carousel/carouselSort.ts` with unit
tests alongside.

## Related fixes shipped with this feature

- `ConversationListContainer`'s vertical scroller now carries
  `overflow-x-hidden` — without it, `overflow-y:auto` computes
  `overflow-x:auto` (see [mobile-kanban-scrolling.md]) and each chat becomes
  a competing horizontal scroller inside the strip.
- Each carousel column wraps its chat in an error boundary: with N
  workspaces rendered at once, one workspace with bad data must not blank
  the entire view (observed: a malformed `executor_action` crashed the whole
  page through the app-level boundary).

## Contributed by

- vk/f9c1-vk-workspace-car
