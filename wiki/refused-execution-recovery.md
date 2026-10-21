# Refused-execution recovery: typed 503, server suggestions, bounded backoff

What happens when a workspace's sticky execution server refuses a follow-up
(PSI pressure, draining, offline, lease expired), and the traps found while
building the chat's recovery banner.

## Contract

- The worker answers a refused dispatch with HTTP 503 (`worker_api.rs`:
  `UnderPressure`, `Draining`). `WorkerClientError::service_unavailable_reason()`
  identifies it. `local-deployment`'s `dispatch_failure_error` maps it to
  `ContainerError::WorkerUnavailable` with `reason: Refusing { detail }`.
  Before this change it was `Other`, which the API rendered as a generic 500.
  The failure evidence JSON is unchanged.
- `WorkerUnavailable` (`cluster/scheduler.rs`) now carries `worker_node_id`.
  `dispatch_liveness` fills it in for offline, expired-lease and unregistered
  workers too.
- `ApiError::WorkerUnavailable` answers 503 with
  `error_with_data_and_message`. `error_data` is a `WorkerUnavailableErrorData`:
  `{ worker_node_id, worker, reason: not_registered|offline|lease_expired|refusing, retryable }`.
  The `message` is still the full human text, for MCP. The conversation log
  still reads `Failed to start execution: Execution server <host> is not
  accepting new executions: <worker reason> Move this workspace …`.
- **`handleApiResponse` used to drop `error_data` on non-2xx responses.**
  Every typed error before this arrived as 200 + `success: false`. It now
  passes `error_data` into `ApiError` for any status. Bodies without
  `error_data` behave as before.

## Frontend shape

- Pure rules live in `shared/lib/workerRecovery.ts`. `isWorkerAccepting` is
  `isWorkerEligible` (the Server Affinity picker's rule) plus no
  `resource_snapshot.admission_refusing`. `rankAlternativeWorkers` excludes
  the refused worker and sorts by `load_1m`, then active executions, then id.
  Backoff is 5 s doubling, capped at 60 s, inside a hard 10-minute window from
  the first refusal. Repeat refusals keep the window and the backoff level.
- `useWorkerUnavailableRecovery` polls `GET /api/worker-nodes`. It does not
  resend blindly: every refused resend leaves another failed execution in the
  conversation. It auto-resends only to the same worker, and only after that
  worker's heartbeat shows it accepting. It never auto-resends on the first
  look, because the heartbeat can predate the refusal. Moving to another
  worker happens only on a click. The click calls the same
  `PATCH /api/workspaces/{id}/affinity` (`restart_running: false`) as the
  affinity panel, then resends.
- The banner (`@vibe/ui` `ServerRecoveryBanner`, `serverRecoveryNotice` prop
  on `SessionChatBox`) covers existing-session follow-ups only. A new session
  is `POST /api/sessions` followed by a follow-up, so resending would create
  a duplicate session. A refused new session shows the typed message only.

## Traps (six Codex rounds)

Any async recovery that sends on the user's behalf needs all of these guards:

- **Session switch mid-request.** A 503 can settle after the user switched
  sessions, and the stable failure callback would then start recovery in the
  *new* session. `onFollowUpFailed` passes the originating `sessionId`, and
  `start` ignores a mismatch. A *successful* late resend must also skip the
  composer cleanup when the session changed.
- **Deadline after an await.** Check the deadline after the worker-list
  request settles, not only when scheduling. A slow request or a suspended tab
  can otherwise auto-send after the advertised window.
- **Dismissal mid-resend.** The resend's own refusal reaches `start` through
  the send's error callback *before* `runResend` resumes and sees the bumped
  generation. Track the in-flight resend's generation and ignore its failure
  when recovery was dismissed.
- **Superseding sends.** Edit-and-retry, queueing and a new send each dismiss
  recovery. An automatic resend also waits while the session has a running
  agent (`canAutoResend`).
- **Don't dismiss on "agent running".** A refused dispatch inserts a process
  row that can render as `running` before it flips to `failed`, so an effect
  keyed on running state dismisses recovery spuriously. Use explicit
  dismisses plus the resend-time guard instead.
- **Drafts.** A successful follow-up deletes the session's `DraftFollowUp`
  scratch on the server. Clear the composer only if its text is unchanged
  since the send. Otherwise save it again, or text typed while waiting is
  lost on reload.

## Not verified live

The tests cover the contract and the hook with mocked APIs. No real pressure
refusal was exercised against a cluster from the task worktree.

## Contributed by

- `vk/bde2-graceful-degrada`
