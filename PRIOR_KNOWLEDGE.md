# Prior Knowledge: Shared Human/Agent Browser Session Control

Task: `vk/57e0-add-shared-human`. The project has two populated knowledge
bases: `wiki/` (indexed in `wiki/INDEX.md`) and `docs/knowledge-base/`. No
page covers browser automation (the feature is greenfield — verified by
repo-wide search), but several pages constrain the subsystems this task
touches.

## Execution-process lifecycle (`wiki/agent-process-lifecycle.md`) — most relevant

- One turn = one `ExecutionProcess` row; every per-execution facility in
  `LocalContainerService` is a `HashMap<Uuid, …>` keyed by execution id. The
  exit monitor finalization path is the reliable "execution completed" hook —
  release browser-control leases there (or via a status watcher), not by
  polling DB state.
- The exit monitor kills **twice** (exit-signal branch + monitor tail with
  `kill_on_drop`). Any long-lived process VK spawns (our Chromium) must NOT
  ride on the coding-agent child machinery; spawn Chromium as a
  service-owned process with explicit process-group kill, mirroring the
  dev-server/pgid substrate (`ExecutionProcess.pgid`, boot re-adoption,
  `kill_orphan_process_group`) rather than `kill_on_drop` alone, which the
  codebase treats as unreliable for grandchildren.
- Registry concurrency traps that transfer directly to a browser-session
  registry: insert-before-remove (no invisibility gap between owners),
  generation-conditional reap in idle sweeps (don't kill a re-registered
  entry), never hold the registry lock across a kill/`try_wait` await, and
  poll-loop watchers must check `tx.is_closed()` or they spin forever against
  a deliberately long-lived child.
- Gate genuinely new runtime behavior behind an env/config flag so default
  behavior is unchanged when the capability is unobservable in CI (keep-warm
  used `VK_KEEP_WARM_AGENTS`; browser sessions should degrade typed-and-clean
  when Chromium is absent).

## Frontend workspace surface (`wiki/workspace-carousel-view.md`)

- The chat/right-main component stack is prop-driven; per-workspace UI state
  lives in the `useUiPreferencesStore` zustand store. Singleton stores with N
  writers clobber each other — a new browser store must be keyed by
  session/workspace id, not global.
- Editor autofocus fires on mount: focus is NOT a user-engagement signal.
  Use real interaction (`onPointerDownCapture`/`onKeyDownCapture`) for
  anything like "user engaged with the live view"; blur is unreliable for
  release — our control-release must key on WS disconnect, not blur.
- Mount-windowing bounds websocket count; debounced effects must compare
  content before re-arming timers or they starve.

## Events/activity (`wiki/kanban-items-state-and-activity-grouping.md`, explorer findings)

- Realtime updates flow from SQLite preupdate/update hooks
  (`services/src/services/events.rs`) that push JSON patches into a shared
  `MsgStore`; per-resource filtered streams live in `events/streams.rs`.
  DB writes automatically broadcast — so persisting browser-session rows and
  audit transitions gives the sidebar list stream almost for free; live-only
  controller state must be pushed explicitly by the service.

## Process/deploy context (`wiki/self-hosted-deployment.md`)

- This fork deploys as versioned releases of extracted binaries; services on
  the deploy host run long-lived. Abandoned Chromium processes would survive
  server restarts — reuse the pgid persistence + boot-adoption substrate for
  cleanup after unclean shutdown.

## Review/process conventions (multiple pages, e.g. `docs/knowledge-base/mcp-connectivity-testing.md`)

- Prior tasks consistently: keep protocol/schema changes minimal by reusing
  existing envelopes (`ApiResponse`, in-band MCP error JSON), clear transient
  frontend state at configuration-invalidating boundaries, and treat
  security-sensitive diagnostics as redact-by-default — matching this task's
  URL/profile redaction requirement for activity events.
- Adversarial (Codex) review has repeatedly caught concurrency-window bugs in
  exactly the kind of registry/lease logic this task adds — budget for that
  in stage 11.
