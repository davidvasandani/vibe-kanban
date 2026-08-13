# Stale Execution Status Follow-up — Technical Specification

## Objective

Eliminate every known path by which the Vibe Kanban composer can remain in a
running/Stop state after work has ceased, while preserving cancellability for
genuinely active coding-agent and lifecycle-script executions. Repair the
snapshot/live-stream handoff contract shared by execution processes and sibling
streams, expose bounded connection failures without losing last-known-good
state, and restore isolated SpecKit records for PR #226 and the earlier task
whose directory it reused.

## Scope

Only the `vibe-kanban` repository is in scope. Homelab deployment and all other
services are explicitly excluded.

The work covers:

- execution-process state derivation consumed by the composer;
- backend snapshot-plus-live streams and broadcast-lag behavior;
- Codex app-server/worker completion, terminal evidence, and reconciliation;
- the shared JSON-patch WebSocket client and remote relay close metadata;
- SpecKit task-directory isolation and its validation;
- focused regression tests and repository verification.

## Functional requirements

### FR-1: One authoritative running-attempt predicate

1. A single runtime helper must determine whether an attempt visible to the
   composer is active.
2. The provider/composer path must call that helper, rather than reimplementing
   its status/reason predicate.
3. A visible `running` execution with run reason `codingagent`, `setupscript`,
   `cleanupscript`, or `archivescript` is active and cancellable.
4. `completed`, `failed`, `killed`, `interrupted`, and `indeterminate`
   executions are not active, for every relevant run reason.
5. Provider-boundary coverage must prove that the value consumed by
   `useWorkspaceExecution` and `SessionChatBoxContainer` comes from this helper.

### FR-2: Lossless snapshot/live handoff

1. Every DB-snapshot-plus-broadcast-patch stream must subscribe to broadcasts
   before beginning its snapshot query.
2. Updates received while the snapshot is being built must be buffered and
   reduced after `snapshot` and `Ready` in publication order.
3. Broadcast receiver lag means the stream has lost authority. It must emit an
   error and terminate so its WebSocket closes and the client reconnects for a
   new snapshot; lag must never be silently swallowed.
4. A deterministic execution-process test must pause after subscription but
   before snapshot completion, publish `running` then terminal state, and prove
   the reduced client state is terminal. The test must fail if query-before-
   subscribe ordering is restored.
5. Tests must assert that lag produces a stream error and an error WebSocket
   close suitable for reconnect/resnapshot.
6. `stream_scratch_raw`, `stream_workspaces_raw`,
   `stream_browser_sessions_for_workspace_raw`, and
   `MsgStore::history_plus_stream` must be audited. Snapshot/live streams must
   use the repaired contract; any exempt API must have explicit evidence and a
   focused test or documentation.
7. Prefer a shared lossless-handoff primitive where stream contracts are
   structurally alike, so subscription order and lag policy cannot diverge.

### FR-3: Bounded authoritative execution finalization

1. Inspect Codex app-server and worker completion paths, execution-worker
   terminal evidence, and all errors or early returns before
   `ExecutionProcess::update_completion`.
2. A normalized final assistant response is evidence that requires bounded
   reconciliation, but is not by itself proof that the child process exited.
3. If an execution has final output but its expected completion/finalization
   event is delayed, lost, or interrupted, reconciliation must seek positive
   process/worker liveness and terminal evidence for a bounded interval.
4. When positive liveness can no longer be established, the authoritative row
   must transition without user cancellation to the most truthful one of
   `completed`, `failed`, `interrupted`, or `indeterminate`.
5. No final assistant response may coexist indefinitely with an authoritative
   `running` execution.
6. Regression coverage must simulate final output followed by missing/delayed
   finalization and prove both the backend terminal transition and the composer
   returning to Send from that authoritative update without refresh or Stop.
7. Every completion update failure must be observable and must not silently
   abandon a running row.

### FR-4: WebSocket connection and retry truthfulness

1. `useJsonPatchWsStream` must track receipt of an authoritative `Ready`
   separately from allocation of its initial client object.
2. Repeated initial connection failures must surface `Connection failed` after
   a bounded number of attempts.
3. Once a valid snapshot/`Ready` has been received, reconnect attempts must
   retain and render that last-known-good snapshot; transport errors must not
   erase it.
4. Backoff must not reset merely because a socket opened if it repeatedly
   closes before establishing an authoritative snapshot. Repeated
   open/lag/error/resnapshot cycles must have bounded load.
5. Remote relay behavior must retain diagnostic server close code/reason even
   where browser APIs cannot legally originate a reserved `1011` close code,
   while still causing reconnect/resnapshot.

### FR-5: SpecKit artifact isolation

1. Recover the prior `vk/5e1e-vk-workspace-cre` task artifacts from git history
   into their original task record.
2. Place PR #226 artifacts in a dedicated directory for
   `vk/3488-fix-stale-execut`.
3. Reconcile the reused `specs/vk/a5f8-concat-repeating` directory so neither
   task's record is lost and unrelated stale content is not attributed to the
   wrong task.
4. Update internal references to the corrected locations.
5. Add or strengthen automated validation so a future SpecKit invocation
   cannot silently target a directory already owned by another task.

## Quality and verification requirements

- Tests must exercise runtime boundaries, not only extracted predicates or
  error-string helpers.
- Race tests must use deterministic synchronization, never timing sleeps.
- Backend tests must reduce emitted `snapshot`, `Ready`, and buffered patches as
  a real consumer would.
- Run focused frontend and backend tests during implementation, followed by
  formatting, type checks, lint, and the applicable broader suites.
- Generated files must be regenerated from their Rust sources if types change.
- An independent Codex review must report no significant findings before the
  task is ready.
- Reusable knowledge must be recorded in the project knowledge base and tagged
  with this task ID before completion.

## Deliverables

1. Shared frontend running-attempt derivation and provider/composer regression
   tests.
2. Shared backend lossless snapshot/live handoff and deterministic race/lag
   tests across all audited streams.
3. Bounded execution finalization/reconciliation and missing-finalization tests.
4. Correct JSON-patch client readiness/error/backoff semantics and relay close
   metadata tests or evidence.
5. Restored and isolated SpecKit artifacts plus collision prevention.
6. Verification evidence, independent review results, knowledge-base update,
   and a merged pull request.

## Non-goals

- Changing homelab modules, hosts, or deployment configuration.
- Treating final normalized output as unconditional proof of successful process
  exit.
- Hiding stream-authority loss by continuing from a potentially incomplete
  patch sequence.
- Broad redesign of the composer or execution UI beyond accurate Send/Stop and
  cancellation behavior.
