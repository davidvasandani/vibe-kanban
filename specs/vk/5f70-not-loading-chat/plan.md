# Implementation Plan: The chat panel always finishes loading

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context

- Frontend only: TypeScript + React in `packages/web-core` (shared by
  local-web and remote-web, so both are in the blast radius, per IV).
- Tests: vitest + jsdom (`pnpm --filter @vibe/web-core test`). Existing
  fake-socket tests live in `shared/hooks/useJsonPatchWsStream*.test.ts(x)`.
- No server, schema, generated-type, or dependency change.

## Root cause (verified)

1. `crates/server/src/routes/execution_processes.rs`: both
   `handle_normalized_logs_ws` and the raw-logs handler `break` on
   `Some(Err(e))` from the log stream and then `socket.close()`. The client
   gets a clean close (1000) with **no** `{"finished": true}`.
2. `packages/web-core/src/shared/lib/streamJsonPatchEntries.ts` settles its
   caller only on `finished` (`onFinished`) or a transport `error` event
   (`onError`). Its `close` listener only resets state, and nothing times out
   a silent socket.
3. In `features/workspace-chat/model/hooks/useConversationHistory.ts`,
   `loadEntriesForHistoricExecutionProcess` wraps that controller in a
   promise, which stays pending forever in the case above.
   `loadProcessesInOrder` awaits `Promise.all` over a slice. So one pending
   fetch blocks the initial-load effect before
   `emitEntries(..., 'initial', false)`, and `ConversationList` keeps
   `loading === true` with no rows. That is the spinner in both screenshots.

Every live measurement taken during the investigation succeeded quickly, so
the failing moment itself was not captured. The trigger is inferred. At the
time, the coordinator was at load 35–49 with only ~56% CPU busy. The rest was
NFS I/O wait (`xprtiod`/`fscache` kworkers, many `git` processes on shared
workspaces), which is when a raw-log read can fail. A proxy or iOS dropping the
socket produces the same client state.

## Architecture & Approach

### A. `streamJsonPatchEntries` settles exactly once (FR-1, FR-2, FR-3, FR-6)

In `shared/lib/streamJsonPatchEntries.ts`:

- Add a `settled` flag. Route every terminal outcome through two local helpers:
  `finish()`, which calls `onFinished` once, and `fail(err)`, which calls
  `onError` once. After settling, clear the idle timer and ignore further
  terminal signals.
- `close` listener: if the stream was not already settled and was not closed
  by the caller, call `fail(new Error('stream closed before finished'))`.
- New optional `idleTimeoutMs` in `StreamOptions`. Arm the timer when the
  stream starts, so it also covers a handshake that never completes, and
  re-arm it on every incoming message. When it fires: close the socket and
  `fail(new Error('stream idle timeout'))`.
- `close()` from the caller sets `closed` (already present) and clears the
  timer, so a deliberate close reports nothing.
- The existing `error` listener, parse-error path, and open-failure path go
  through `fail`.

### B. Callers (FR-4, FR-5)

In `features/workspace-chat/model/hooks/useConversationHistory.ts`:

- `loadEntriesForHistoricExecutionProcess` passes
  `idleTimeoutMs: HISTORY_STREAM_IDLE_TIMEOUT_MS` (30 000, a new constant in
  `shared/hooks/useConversationHistory/constants.ts`). A rejection is already
  counted and skipped by `loadProcessesInOrder`. The initial load then
  completes, and the turn stays unloaded, so `hasUnloadedHistoricProcesses()`
  keeps "load earlier" available (FR-4, clarification Q2).
- `loadRunningAndEmit` passes no idle timeout (FR-5). Its `onError` already
  rejects, which `loadRunningAndEmitWithBackoff` retries. The only new
  behaviour is that a close without `finished` now reaches that path.

No change to `ConversationListContainer.tsx`. Its `loading` flag clears on the
first `'initial'` emit with `loading: false`, which now always happens.

### C. Tests (II, XL)

New file `shared/lib/streamJsonPatchEntries.test.ts` (jsdom, mocking
`openLocalApiWebSocket` like the existing reconnect test, with a
`FakeSocket` that supports `addEventListener`). One case per terminal path:

1. `finished` → `onFinished` once with the entries; the following close
   reports nothing.
2. Clean close (1000) without `finished` → `onError` once.
3. `error` then `close` → `onError` once.
4. No messages within `idleTimeoutMs` → `onError` once and the socket is
   closed. Messages arriving more often than the deadline → no error.
5. Caller `close()` then socket `close` → nothing reported.
6. No `idleTimeoutMs` → silence never errors (live-stream behaviour).

## Data Model

No persisted data changes. See `./data-model.md`.

## Contracts

See `./contracts/stream-json-patch-entries.md` for the settlement contract of
`streamJsonPatchEntries`.

## Research Notes

See `./research.md`.

## Constitution Check

- **XL (new)**: every awaited client stream settles exactly once on sentinel,
  close without sentinel, error, or idle deadline, and settled-history reads
  are the only ones with a deadline. That is exactly A + B.
- **XXXIX**: server termination is unchanged and already satisfied for
  successful reads. The server's clean-close-on-error is left alone (out of
  scope), and the client no longer depends on it.
- **II**: every terminal path has a test (C).
- **III**: two files of logic plus one constant. It reuses the existing
  skip-and-count logic and the "load earlier" affordance. No new UI.
- **IV**: `packages/web-core` change. Both frontends pick it up, and the
  behaviour is identical in both.
- **XXX**: a failed turn is not faked. It stays unloaded and retryable.

No deviations.

## Risks & Dependencies

- **False idle timeout on a very slow first byte.** The deadline is 100×
  the worst latency measured under load. A turn that times out stays
  retryable, so the cost is a click, not data loss.
- **Live stream now retries on a clean close.** If the server ends a live
  stream without `finished` when the turn ends, the backoff reconnects once to
  a now-completed turn, which replays history and sends `finished`. That is
  bounded (at most 20 × 500 ms) and self-terminating.
- The fix does not make the server's log read succeed under NFS pressure. It
  only makes the UI survive a failed read.
