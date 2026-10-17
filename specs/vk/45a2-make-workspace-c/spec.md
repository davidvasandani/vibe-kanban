# Feature Specification: Workspace chat loads fast when every WebSocket handshake is expensive

**Feature dir**: `specs/vk/45a2-make-workspace-c/`
**Status**: Draft
**Task**: `vk/45a2-make-workspace-c`

## Summary

Opening a workspace chat on the phone takes 40 s to 2 min, but the server
answers every stream the chat needs in under 120 ms. The time goes into
connection setup. Each WebSocket is a new TCP and TLS handshake through
Cloudflare, currently about 0.5 s and up to 10 s, and iOS Safari opens them one
at a time. A workspace page opens about 29 sockets. The chat's history sockets
come last, and each completed turn is loaded twice. This feature makes the chat
depend on as few sockets as possible:

- completed turns load over plain requests, which share one connection;
- identical live streams are shared;
- history is fetched once;
- panels the user cannot see stop queueing ahead of the chat.

Principle XLIV (constitution 0.41.0) records the rule.

## User Stories

- As a phone user opening a workspace, I want the conversation to appear
  within a few seconds, not after a minute, so that I can read and reply on
  the go.
- As a user whose network is slow or flaky, I want a turn that fails to load
  to be skipped and retryable, so that one bad request never leaves the chat
  spinning.
- As a user watching an agent work, I want the running turn to keep streaming
  live, and to end up complete once it finishes, so that I never see a
  truncated answer.
- As an operator, I want each page load to make far fewer connection upgrades,
  so that the edge proxy and the coordinator see less churn.

## Functional Requirements

- **FR-1: Completed turns load by request.** A turn that is not running when
  the chat loads is fetched with one plain request per turn. The request
  respects the selected host and the remote relay. It returns the same
  conversation content the live replay would converge to.
- **FR-2: The response says whether it is settled.** Each history response
  states whether it is complete. An incomplete answer is never shown or cached
  as final. The chat then falls back to the live path, which converges to the
  full log. This covers a turn that finishes between the process snapshot and
  the fetch.
- **FR-3: Running turns stay live.** A running turn keeps its live stream and
  existing reconnect behaviour. When it finishes, its final content is
  reloaded once.
- **FR-4: Requests are bounded.** Every history request has a deadline. A
  timeout, an abort, an error status or a malformed body counts as a failure,
  never as completion. Each request settles exactly once. A failed turn is
  skipped and counted, with no automatic retry during the initial load, and
  stays loadable through "load earlier".
- **FR-5: Cold server work survives an abort.** If the client gives up while
  the server is building a turn's settled log for the first time, the server
  still finishes and stores it, so the next attempt is fast. That work stays
  under the existing concurrency limits.
- **FR-6: History is fetched once per scope.** Within one chat scope
  (workspace and session), each completed turn is requested at most once.
  Responses fetched ahead of the visible window are kept and used when the
  user (or the top sentinel) asks for earlier history.
- **FR-7: One process-list stream per session.** All parts of the page that
  need a session's process list share one live connection, whether or not they
  want soft-deleted processes. Filtering happens on the client.
- **FR-8: Identical live streams are shared.** Components subscribing to the
  same live stream on the same host share one connection. The connection stays
  open briefly after the last subscriber leaves, so that a remount does not
  reconnect. Requests that are equivalent on the server, such as executor
  discovery for a session with or without a redundant workspace id, use the
  same stream.
- **FR-9: The chat goes first on mobile.** On the mobile layout, the preview
  and browser panels do not connect until their tab is first shown. The
  workspace diff stream does not connect until a diff tab is shown or the
  chat's history has loaded. Desktop behaviour is unchanged.
- **FR-10: Reconnect behaviour is preserved.** Shared streams keep today's
  behaviour:
  - the last snapshot stays rendered while the stream reconnects;
  - a fresh snapshot replaces it after reconnect;
  - only an authoritative ready signal resets backoff;
  - `finished` is terminal;
  - a clean close does not reconnect.

## Out of Scope

- The workspace summaries path, which was fixed in #350.
- NFS and Cloudflare configuration.
- Multiplexing all subscriptions over one connection. This is written up as a
  proposal only.
- Rendering the first chat view from a request-based process snapshot before
  the live stream attaches (stretch goal, not built).
- Changing which turns the initial window shows, or the paging thresholds.

## Acceptance Criteria

- [ ] For workspace `14312466-…` with the chat fully loaded: zero
      `normalized-logs/ws` connections for completed turns, with history
      arriving as requests.
- [ ] Exactly one `execution-processes/stream/session/ws` per page load.
- [ ] Each completed turn's history is requested once.
- [ ] The chat spinner does not wait on git diff, discovered-options,
      approvals, preview or browser-session sockets.
- [ ] Before and after WebSocket counts and time until rows, recorded with a
      mobile-viewport probe, without added latency and with about 150 ms RTT.
- [ ] Automated tests cover the history request in five cases:
      - success;
      - timeout;
      - error status;
      - abort;
      - finished during load.
- [ ] Automated tests cover one connection shared by several session-stream
      consumers, and a single history load per scope.
- [ ] Server tests cover the new history responses: settled versus live
      source, and incomplete reads.
- [ ] The knowledge base records the handshake-cost rule and the numbers.

## Clarifications

Resolved in `/speckit.clarify` (2026-10-01). No answers were supplied with the
command. Each question below was decided from the task text, the existing
constants, and the baseline probe.

- **History request deadline is 30 s, total.** This matches the existing
  `HISTORY_STREAM_IDLE_TIMEOUT_MS`, so the HTTP and socket paths fail on the
  same scale. A request has no progress signal to reset an idle timer, so the
  bound is total. FR-5 makes this safe for cold logs: the server keeps
  building and stores the result, so "load earlier" succeeds on retry. Healthy
  reads finish in under 120 ms, so 30 s only bounds the failure case.
- **Script turns move to requests as well.** The chat loads them through the
  same history path (`raw-logs/ws`). Leaving them on sockets would keep a
  handshake per setup or cleanup turn, against FR-1 and XLIV. Raw output
  becomes the same `STDOUT`/`STDERR` entries the socket produces.
- **On mobile, the diff stream starts when a diff tab is shown or the chat's
  initial history settles, whichever comes first.** The chat header's
  diff-stats pill reads that stream. Waiting for a diff tab alone would leave
  the pill empty until the user leaves the chat. Starting after the history
  settles keeps the 613 KB, slow-first-frame stream out of the chat's critical
  path, which is what the task asks for. Once started, it stays connected for
  that workspace.
- **A shared stream lingers 3 s after its last subscriber leaves.** In the
  probe, the churn (approvals and discovered-options closing and reopening)
  happened within about 150 ms of mounting. 3 s absorbs remounts and quick tab
  switches, while a stream nobody needs still closes promptly.

## Open Questions

None remain.
