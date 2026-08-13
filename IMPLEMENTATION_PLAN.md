# Implementation Plan: Stale Execution Status Follow-up

## 1. Preserve baseline and reconstruct artifact ownership

1. Record the current branch, dirty tree, PR #226 merge ancestry, and the
   originating task commits.
2. Locate the last correct versions of the `vk/5e1e-vk-workspace-cre` artifacts
   and identify exactly which files PR #226 overwrote in
   `a5f8-concat-repeating`.
3. Establish `specs/vk/3488-fix-stale-execut` as this feature lineage's proper
   SpecKit directory, restore the earlier task to its own directory, and remove
   or restore each stale file in `a5f8` according to git evidence.
4. Find the pipeline/task-directory selection code and tests, then add an
   ownership/collision check before any generator writes.

## 2. Unify the composer activity source of truth

1. Move `hasRunningAttempt` to a neutral frontend module if importing it from a
   hook would create an awkward provider dependency.
2. Replace `ExecutionProcessesProvider`'s inline visible-process predicate with
   the shared helper.
3. Add a provider consumer harness covering visible versus dropped processes,
   all four cancellable run reasons, and every terminal status.
4. Exercise `useWorkspaceExecution` or the nearest composer consumption
   boundary so the asserted Stop/Send result is the actual runtime context
   value, not only a pure-helper result.

## 3. Build and apply a lossless stream handoff primitive

1. Inventory the execution-process, scratch, workspace, browser-session, and
   message-history stream construction paths, their snapshot sources, filter
   semantics, and WebSocket adapters.
2. Extract the smallest shared primitive/policy that takes a receiver acquired
   before an awaited snapshot, emits snapshot plus `Ready`, drains buffered live
   messages, and maps broadcast lag to `io::Error` followed by termination.
3. Convert the execution-process stream first.
4. Add a deterministic synchronization hook/harness that pauses after receiver
   subscription and before snapshot completion; publish running→terminal in the
   gap and reduce the complete output to prove terminal state.
5. Assert the same test fails if receiver acquisition is moved below the query.
6. Add an intentionally small-capacity lag test and route its error through the
   real execution-process WebSocket handler, asserting a retryable error close.
7. Convert scratch, workspace, and browser-session snapshot streams to the same
   contract with focused filter/state-transition tests.
8. Change `MsgStore::history_plus_stream` to subscribe before reading history
   and make lag fatal. Add a concurrency test proving handoff ordering and a lag
   test; audit downstream consumers for their intended reaction to the new
   error.

## 4. Repair client readiness, retention, and retry pressure

1. In `useJsonPatchWsStream`, represent separately: allocated initial object,
   authoritative `Ready` received for the endpoint, current transport-open
   state, and consecutive unhealthy reconnects.
2. Surface the bounded initial connection error based on missing authoritative
   readiness, including failures thrown before a WebSocket object is returned.
3. Preserve data and initialized UI after a prior `Ready` while reconnecting;
   clear only for endpoint/identity changes.
4. Reset backoff only after a connection reaches `Ready` (or an equivalent
   healthy threshold), not on `open` alone. Test repeated open→error-close
   cycles and subsequent healthy recovery with fake timers.
5. Trace local and remote WebSocket error-close handling. Where a relay cannot
   send reserved close code `1011` from browser JavaScript, encode/preserve the
   server code and reason through a legal transport path and test both
   diagnostics and reconnect behavior.

## 5. Close backend execution-finalization gaps

1. Trace Codex app-server events from normalized final assistant output through
   child exit observation, worker journal/protocol terminal events, container
   task cleanup, and every `update_completion` call.
2. Enumerate every error, cancellation, dropped future, missing child handle,
   disconnected worker, and early return that can strand a running row.
3. Reconcile this with startup orphan/WIP preservation so terminalization never
   bypasses required repository capture.
4. Introduce a bounded reconciliation trigger when a final assistant response
   is observed but terminal evidence has not arrived. Keep output evidence and
   process-exit evidence distinct.
5. During the bound, accept authoritative worker/process liveness and normal
   terminal events. At expiry with no positive liveness, preserve recoverable
   work as required and write the most truthful non-running status, defaulting
   to `indeterminate` when success/failure cannot be proven.
6. Make terminal update failures observable and retry them within a bound or
   enqueue them for an existing durable reconciliation path.
7. Add deterministic tests for normal completion, delayed completion, lost
   finalization, interrupted reconciliation, positive-liveness continuation,
   missing-liveness expiry, and update failure.
8. Add an end-to-end state test showing final output plus missing completion
   converges through the authoritative execution stream and returns the
   composer to Send without refresh or manual Stop.

## 6. Verify incrementally

1. Run focused Vitest suites for activity derivation, provider/composer
   behavior, and WebSocket reconnection.
2. Run focused Rust tests for each stream contract, WebSocket error closure,
   Codex finalization, and reconciliation.
3. Run `pnpm install --frozen-lockfile` if this worktree has not been prepared.
4. Run `pnpm run format`, relevant generated-type checks, `pnpm run check`,
   `pnpm run lint`, and the applicable Rust workspace tests.
5. Inspect the final diff for unintended edits, especially generated files,
   SpecKit history, and any homelab path.

## 7. Independent review and delivery

1. Run the repository's independent Codex review workflow against the complete
   diff.
2. Confirm each significant finding against code, fix confirmed issues, rerun
   focused and broad verification, and repeat review until clean.
3. Update the project knowledge base with reusable completion-reconciliation,
   lossless-handoff, retry-health, and SpecKit-isolation knowledge; tag this
   task and refresh the index.
4. Commit the knowledge-base update and implementation intentionally.
5. Push the task branch, open a PR against the current base branch, wait for
   required checks, address failures without leaving scope, and merge.
