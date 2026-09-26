# Awaited stream settlement (chat history loading)

The chat's conversation view loads completed turns by opening
`/api/execution-processes/{id}/normalized-logs/ws` (`raw-logs/ws` for
scripts) through `streamJsonPatchEntries` and waiting for `{"finished": true}`.
`useConversationHistory` wraps each fetch in a promise, and
`loadProcessesInOrder` awaits them in `Promise.all` slices. The first
`'initial'` emit, which clears `ConversationList`'s spinner, happens only after
those promises settle. So **one unsettled fetch holds the whole chat behind
its spinner**, with nothing to recover it.

## Rules

- **A close is never completion.** The server closes cleanly (code 1000)
  *without* `finished` when the log stream errors (both handlers in
  `crates/server/src/routes/execution_processes.rs` `break` and then
  `socket.close()`). Cloudflare and iOS also drop sockets. A waiter that
  settles only on `finished` or on the `error` event hangs forever on a clean
  close.
- **Exactly once.** `streamJsonPatchEntries` settles through a `settled`
  guard: `finished` → `onFinished`, anything else (close, error, parse error,
  open failure, idle timeout) → `onError`, and later signals are ignored.
  Closing by the caller reports nothing. If a timeout fires while
  `openLocalApiWebSocket` is still pending, close the late socket when it
  arrives, or it leaks.
- **Idle deadline only for settled history.** Pass `idleTimeoutMs`
  (`HISTORY_STREAM_IDLE_TIMEOUT_MS`, 30 s, reset per message) for completed
  turns. Never pass it for a running turn: a thinking agent is silent for
  minutes, and that stream has its own `loadRunningAndEmitWithBackoff`
  reconnect loop. With close-as-error, a dropped live stream now reaches that
  loop.
- **Degrade instead of blocking.** A failed turn is skipped and counted by
  `loadProcessesInOrder` and remains unloaded, so "load earlier" retries it on
  demand. Do not auto-retry during initial load: failures cluster when the
  coordinator is already struggling.

Principle: constitution XL (client side), the counterpart to XXXIX (server
reads terminate on their own).

## Debugging recipe: "chat spinner never goes away"

- The public origin is behind Cloudflare Access and not reachable from
  workers. Talk to the coordinator directly at `http://172.16.100.102:3334`
  (the frontend is served there too).
- Node 24 on workers has global `fetch` and `WebSocket` (no curl or python).
  Time every log socket for a session: get the process snapshot from
  `/api/execution-processes/stream/session/ws?session_id=…` (wait for
  `Ready`), then open each process's `normalized-logs/ws` and record
  time-to-first-message and time-to-`finished`. Healthy is < 300 ms, even for
  logs over 1 MB.
- Reproduce the mobile layout in the managed browser (`browser_evaluate`):
  `document.write` a 390 px-wide same-origin `<iframe>` of
  `/workspaces/<id>`. `useIsMobile` follows the iframe's viewport, so you get
  the phone tab layout. Poll `.animate-spin` inside it for timing.
- `GET /api/cluster/metrics` → coordinator `latest.cpu.load_*`,
  `total_busy_percent`, and `processes`. A load far above the core count with
  low CPU busy, plus `kworker … xprtiod`/`fscache` and many `git -C
  /srv/vibe-kanban-shared/…` processes, means NFS I/O wait. That is when log
  reads fail or crawl. Note that your own headless browser also shows up
  there.
- Three identical spinners can occupy the chat body: `WorkspacesMain`'s
  `isLoading` (workspace-list streams or record not ready), its
  `showLoadingOverlay`, and `ConversationList`'s `loading` (history not
  settled). The chat box renders in every case, so it does not tell them
  apart. Check the DOM instead: only `ConversationList`'s spinner sits inside
  the `w-chat` conversation wrapper.

## Contributed by

- vk/5f70-not-loading-chat
