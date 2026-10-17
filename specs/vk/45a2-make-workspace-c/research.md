# Research: workspace chat load over expensive handshakes

## Baseline probe (2026-09-30, deploy `f4b01e4`)

Mobile Chromium (iPhone 13) against `http://172.16.100.102:3334`, workspace
`14312466-…`, session `f2b4a22a-…`, 6 completed turns. Rows rendered at
2.1 s with no added latency. 29 sockets opened before the rows appeared:

- 10 `normalized-logs/ws`
- 3 session process streams
- 4 discovered-options
- 3 approvals
- the git diff stream (no first frame within the 5 s window)
- 3 workspace/preview scratch streams
- 1 browser-session stream

## Decisions

### D1. New GET routes rather than `/messages`
`/messages` projects entries into `SessionMessage` (role-filtered, text
truncated to 4000 characters, tool, diff and stdout entries dropped). The chat
needs the full `PatchType` entries. Extending `/messages` would mix two
contracts in one response. **Rejected.**

### D2. The `complete` flag instead of status polling
The race is a process that finishes between the snapshot and the fetch, while
its `MsgStore` may still be live. The server already knows whether it served
a live history snapshot or a settled source, so it says so. The client falls
back to the socket, which converges because the store's tail ends when the
store drops. Alternatives:
- Retry the GET with backoff. This guesses at how long finalization takes.
- Block the server until the store drops. This is a request-scoped wait on a
  live tail, which XXXIX forbids.

### D3. Detached drain on the server
`HistoricalNormalizationLifetime` aborts the normalizer when the stream is
dropped. A browser timeout would therefore cancel a cold normalization every
time, and the turn would never become cheap. Running the drain in
`tokio::spawn` lets it finish and write the sidecar. The per-execution lease
and the global permit of 1 still bound the work, so aborted retries cannot
multiply it.

### D4. Sharing in `useJsonPatchWsStream`, not per call site
Sharing in the hook fixes the session process stream, the identical
discovered-options pair and the approvals churn in one place. Per-call-site
providers would each need their own lifecycle. Alternative considered:
lifting the process stream into `ExecutionProcessesProvider` only. That fails
because `WorkspacesLayout` sits *above* the provider and `BrowserPanelContainer`
is outside it.

### D5. A linger, not deferred opening
Mount/unmount churn (approvals) would be absorbed by either. The linger also
keeps the data warm for a quick remount, and `useSyncExternalStore`'s
StrictMode double-subscribe never reaches a socket. 3 s, from clarify.

### D6. Script turns over HTTP too
From clarify. `raw-logs` uses the same snapshot shape. Script processes are
loaded from stored raw messages, or from a history snapshot if they are live.

### D7. Mobile diff stream gated on "diff tab or chat settled"
From clarify. The chat's diff-stats pill reads the stream, so it opens after
the rows render. Desktop is unchanged.

## Proposal (not built): one multiplexed socket

A single `/api/streams/ws` would carry `subscribe {id, path}` and
`unsubscribe {id}` frames and tag each `JsonPatch`, `Ready` and `finished`
with its subscription id. One handshake would then serve every live stream on
a page. It needs:

- per-subscription backpressure and lag-resnapshot (the current per-socket
  1011 close becomes a per-subscription `resnapshot` frame);
- auth and host scoping per subscription;
- relay/WebRTC transport support for the new frame types.

It is worth it only if HTTP history plus sharing still leaves the chat queued
behind other live sockets on the phone.

## Measurements before and after (2026-10-01, task T051)

Method: a local proxy (`/tmp/probe/proxy.js`) sits in front of the
coordinator's real data (workspace `14312466-…`, 6 completed turns). In "old"
mode it forwards everything, including the deployed `f4b01e4` frontend. In
"new" mode it serves this branch's production build of `local-web`, forwards
`/api` HTTP and WebSocket traffic, and emulates the not-yet-deployed
`GET …/normalized-logs` by draining the coordinator's existing `…/ws` replay
on the proxy side. Both modes pay the same proxy hop. Latency is injected in
the proxy: `RTT_MS` on every request and upgrade, plus `WS_HANDSHAKE_MS` on
each WebSocket upgrade, which models the new TCP+TLS handshake through
Cloudflare. The probe is Playwright with the iPhone 13 profile; it polls
`.animate-spin` inside `.w-chat` until rows render. WebKit is Playwright
1.60's build, run headless with Mesa software EGL.

| Run | Rows rendered | Sockets before rows | `normalized-logs/ws` | Session streams | History GETs |
| --- | --- | --- | --- | --- | --- |
| Chromium, no added latency, old | 3.4 s | 23 | 5 | 3 | 0 |
| Chromium, no added latency, new | 3.6 s | 11 | 0 | 1 | 5 |
| Chromium, 150 ms RTT + 500 ms/handshake, old | 14.7 s / 14.6 s | 19 | 5 / 10 | 3 | 0 |
| Chromium, 150 ms RTT + 500 ms/handshake, new | 7.0 s / 6.2 s | 9 / 8 | 0 | 1 | 5 / 6 |
| WebKit, 150 ms RTT + 500 ms/handshake, old | 6.0 s / 6.1 s | 24 | 10 | 3 | 0 |
| WebKit, 150 ms RTT + 500 ms/handshake, new | 4.1 s / 4.1 s | 11 | 0 | 1 | 5 |

Direct to the coordinator before the change (no proxy, Chromium): rows at
2.1 s, 29 sockets, 10 `normalized-logs/ws`, 3 session streams, 4
discovered-options and 3 approvals.

Notes:
- With no handshake cost, the change saves sockets, not time. The gain
  appears once each socket costs a handshake, which is the phone's situation.
- In the new build, each GET is issued once per turn. 6 GETs means the top
  sentinel also loaded the oldest turn; the 5 it already had were not
  refetched. The old build fetched the same turns twice (10 sockets).
- Still on the chat's critical path: the shared session stream and the
  workspace-list streams (`workspaces/streams/ws`), whose first frame took
  1.3–2.2 s on the server. `WorkspacesMain`'s `isLoading` waits on those
  streams. This task did not change them; they are the next candidate.
- The git diff socket now opens after the history settles (3.5 s), not
  before. The preview-settings and browser-session sockets do not open on the
  chat tab.
- One bare `discovered-options/ws?executor=…` remains. It opens before the
  session id is known and closes when the session-scoped one replaces it.
- Playwright WebKit on Linux is not iOS Safari, and the proxy does not
  serialize WebSocket connects. The real phone check is the Caddy access log
  after deploy (see the task's "After deploy" section).
