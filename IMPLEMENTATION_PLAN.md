# Implementation Plan: Show Sent Chat Messages Without Refreshing

## 1. Confirm the creation-notification race

1. Trace the existing-session send from `SessionChatBoxContainer` through
   `useSessionSend` and `sessionsApi.followUp`.
2. Confirm that the returned `ExecutionProcess` is currently discarded and
   conversation discovery depends exclusively on the session process
   WebSocket.
3. Preserve the current backend and composer success/failure contracts.

## 2. Add an execution-process reconciliation boundary

1. Extend `ExecutionProcessesContext` with a function that accepts a
   server-returned execution process for the provider's active session.
2. Keep response-backed processes in provider-owned state scoped to the current
   session.
3. Build one ID-keyed projection where stream values supersede response-backed
   values for matching IDs.
4. Ignore processes whose `session_id` does not match the active provider
   session.
5. Remove or harmlessly shadow response-backed values once the stream contains
   the same IDs, preventing duplicate rows and subscriptions.

## 3. Reconcile successful sends

1. Allow `useSessionSend` to receive a successful-follow-up callback.
2. Pass the exact `ExecutionProcess` returned by
   `sessionsApi.followUp` to that callback before reporting send success.
3. Wire the chat container to the reconciliation function from its existing
   execution-process provider.
4. Retain existing composer clearing, attachment clearing, error handling, and
   new-session behavior.

## 4. Add regression coverage

1. Test provider reconciliation before stream delivery.
2. Test stream-after-response and stream-before-response races, asserting one
   process per ID and stream authority over newer values.
3. Test session mismatch rejection and provider reset on session change.
4. Test `useSessionSend` invokes reconciliation only for successful existing
   session follow-ups and preserves failure behavior.
5. Where practical, assert conversation history observes the reconciled
   process and opens only one live process stream.

## 5. Verify the change

1. Install locked dependencies if the worktree is not already prepared.
2. Run focused Vitest suites for execution processes, the provider, send logic,
   and conversation history.
3. Run Prettier/formatting and relevant frontend type/lint checks.
4. Run the repository-required broader checks in proportion to the frontend-only
   change and record any unrelated pre-existing failures.

## 6. Review, document, and ship

1. Run an independent Codex diff review and address every confirmed significant
   finding; repeat until clean.
2. Add the reusable HTTP-response/stream reconciliation pattern to the project
   knowledge base with this task ID and refresh its index.
3. Commit the knowledge-base update.
4. Push the task branch, open a pull request against the base branch, monitor
   its checks, address failures, and merge the pull request.
