# Awaited stream settlement (chat history loading)

The chat loads completed turns with plain HTTP requests:
`GET /api/execution-processes/{id}/normalized-logs` (`raw-logs` for scripts),
through `fetchProcessLogSnapshot` in
`shared/hooks/useConversationHistory/fetchHistoricEntries.ts`. It falls back
to the `…/ws` replay (`streamJsonPatchEntries`, awaited until
`{"finished": true}`) only when the server says its snapshot is not settled.
`loadProcessesInOrder` awaits these fetches in `Promise.all` slices, and the
first `'initial'` emit, which clears `ConversationList`'s spinner, happens
only after those promises settle. So **one unsettled fetch would hold the
whole chat behind its spinner**, with nothing to recover it. The rules below
exist to prevent that.

## Each WebSocket is a handshake; history goes over HTTP

Every WebSocket is a new TCP and TLS handshake through Cloudflare. Plain
requests reuse the page's one HTTP/2 or HTTP/3 connection. iOS Safari
connects WebSockets **one at a time**. Since 2026-09-28, Cloudflare has served
this zone from distant data centres (Marseille, Sydney, Copenhagen), so a
handshake costs about 0.5 s and up to 10 s. Phone chat loads took 40 s to
2 min while every stream finished in under 120 ms on the coordinator: a
workspace page opened about 29 sockets, and the chat's history sockets came
last. Rules (constitution XLIV):

- **A settled read is a request, not a socket.** Completed turns use the GET
  above, through `makeLocalApiRequest`, which keeps `/api/host/{id}` scoping
  and the relay/WebRTC transport. Only running turns keep
  `loadRunningAndEmitWithBackoff` over a socket.
- **The response says whether it is settled.** `complete` is false while the
  process has a live `MsgStore` or a `running` row, or when the settled
  source ended without `Finished`. The client then uses the socket replay,
  which converges when the store drops. A process that finished between the
  process snapshot and the GET therefore ends up complete, not truncated.
- **The server drains on a spawned task.** Historical normalization is
  cancelled when its stream drops, so an aborting client would otherwise
  cancel a cold normalization every time and the sidecar would never be
  written. The per-execution lease and the global permit still bound it.
- **One socket per stream identity.** `useJsonPatchWsStream` subscribes to
  `shared/lib/sharedJsonPatchStream.ts`, a registry keyed by endpoint plus
  host (the host comes from `useHostId()`, because the module-level
  `getCurrentHostId()` is updated only in a layout effect after render). The
  socket lingers 3 s after its last subscriber, which absorbs remount churn.
  `useExecutionProcesses` always asks for `show_soft_deleted=true` and
  filters `dropped` locally, which is exactly what the server's `false`
  does. Discovery URLs drop `workspace_id`/`repo_id` when `session_id` is
  set, because the server ignores them then.
- **Fetch each settled turn once per scope.** `loadProcessesInOrder` fetches
  a whole slice but keeps responses only up to the threshold. The top
  sentinel then fires "load earlier" as soon as a short window renders, so
  the discarded turns used to be fetched again. `createSettledEntriesCache`
  keeps them for the scope.
- **Hidden mobile panels wait.** Preview and browser panels mount on first
  visit. The workspace diff stream (613 KB; first frame 15 s cold, 42 s on the
  phone) waits for a diff tab or for the chat's first settled history
  (`useChatHistoryReadyStore`).

Measured on workspace `14312466-…` (6 turns), through a local proxy that
adds 150 ms RTT and 500 ms per socket handshake: Chromium rows went from
14.6 s to 6.2–7.0 s, and Playwright WebKit from 6.1 s to 4.1 s. Sockets before
rows went from 19–24 to 8–11, `normalized-logs/ws` from 5–10 to 0, and session
streams from 3 to 1. With no handshake cost the time is unchanged, so do not
expect a desktop speed-up. Method and table:
`specs/vk/45a2-make-workspace-c/research.md`. After deploy, the real check is
the Caddy access log on think2: the gap from `GET /api/workspaces/{id}` to the
last history GET, and the count of `101` upgrades per load.

## Settlement rules (socket and HTTP)

- **HTTP: a deadline, and every non-success is a failure.**
  `fetchProcessLogSnapshot` uses an `AbortController` deadline
  (`HISTORY_HTTP_DEADLINE_MS`, 30 s, total). Timeout, caller abort, transport
  error, non-2xx, `success: false` and a body that is not a snapshot all
  reject, through one `settled` guard, so a late response cannot settle the
  request twice. None of them is an empty, finished turn.
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
- vk/45a2-make-workspace-c
