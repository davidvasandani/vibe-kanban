# Graceful degradation when an execution server refuses work

## Problem

A follow-up in a clustered workspace fails when the workspace's sticky
execution server refuses the dispatch, for example:

```
Failed to start execution: Execution server think4 (96eca719-…): worker rejected
request with status 503 Service Unavailable: worker is under io pressure
(full avg60 58.9% >= 40%); not admitting new executions. Wait for pressure to
fall below the threshold, then retry.
```

Today the coordinator wraps that refusal in `ContainerError::Other`, so the
HTTP response is a generic 500 ("An internal error occurred"). The chat box
shows that text, and the user has to work out on their own that they should
move the workspace in the Server Affinity panel or keep retrying by hand.

## Goal

When the assigned server cannot take a new execution:

1. The frontend explains what happened and **suggests an alternative execution
   server** that can continue the session. One action moves the workspace there
   and resends the message.
2. If **no alternative is available**, the frontend says so and **keeps
   polling** with exponential backoff for up to **10 minutes**. When the
   original server recovers, it resends the message. If an alternative appears
   first, it suggests that server. After 10 minutes it stops and says so.

## Requirements

### Backend (coordinator)

- R1. A worker dispatch rejected with HTTP 503 (pressure admission refusal,
  release draining) becomes a typed `WorkerUnavailable` error with reason
  `Refusing`, not `ContainerError::Other`. The message names the server and
  includes the worker's own reason.
- R2. `WorkerUnavailable` carries the worker's node id when it is known. The
  `/follow-up` (and every other) API response for it is HTTP 503 with a typed
  `error_data` payload: `{ worker_node_id, worker, reason, retryable }`.
  `message` still holds the human-readable text for MCP and other clients.
- R3. Admission thresholds, replay-before-admission, the single immediate
  dispatch retry, sticky placement and failure evidence stay as they are.
  The coordinator still does no automatic migration and no extra server-side
  retries.
- R4. The stderr log line in the conversation still starts with
  `Failed to start execution:` and names the server.

### Frontend

- R5. `handleApiResponse` keeps `error_data` on non-2xx responses, so callers
  can match on typed 503 data.
- R6. When a follow-up in an existing session fails with a `WorkerUnavailable`
  error, the chat box shows a recovery banner in place of the plain error. A
  refused first message of a new session shows the typed message only, because
  the session already exists and resending would duplicate it.
- R7. Alternative servers are workers that pass the same eligibility check as
  the Server Affinity picker (online, healthy mount, live lease, supports the
  session's executor profile), are not reporting `admission_refusing`, and are
  not the server that refused.
- R8. With alternatives available, the banner lists them, best first (lowest
  load). "Move & resend" calls the existing affinity API
  (`requested_worker_node_id`, `restart_running: false`) and then resends the
  same prompt. A failed move shows the API's message and keeps the banner.
- R9. With none available, the banner says no other server can take the
  session and polls the worker list with exponential backoff (5 s doubling,
  capped at 60 s) until 10 minutes after the first failure. Each poll:
  - if the refusing server is now eligible and not refusing, resend
    automatically (a repeat refusal continues the same 10-minute window);
  - if an alternative has become eligible, switch to the suggestion state
    (polling continues so the original can still recover).
- R10. When the window expires, the banner says the server is still
  unavailable and offers a manual "Retry now". The user can dismiss the banner
  at any time, which stops polling. Recovery state is per session, and
  switching sessions resets it.
- R11. On a successful resend the editor is cleared the same way a normal
  successful send clears it. The resent prompt is the exact prompt that failed.
- R12. All new strings are translated in every locale under
  `packages/web-core/src/i18n/locales/*`.

## Non-goals

- No change to worker admission, scheduler scoring or server-side retry.
- No automatic migration without the user's action. Moving a session to
  another server is always a user choice. The only automatic action is
  resending to the *same* server once it recovers.
- Workspace creation and other dispatch paths (scripts, reviews) keep their
  current error display. They benefit from the typed 503 but get no banner.

## Acceptance

- Rust unit tests: a 503 dispatch rejection maps to
  `WorkerUnavailable { reason: Refusing, worker_node_id }`; the API response is
  503 with typed `error_data` and a message that names the server.
- Vitest: the recovery logic picks eligible, non-refusing alternatives; backoff
  delays double and cap; it resends when the original recovers and stops at
  the 10-minute deadline.
- `pnpm run check`, `pnpm run lint`, `cargo test` for affected crates,
  `pnpm run generate-types:check` and `pnpm run format` all pass.
