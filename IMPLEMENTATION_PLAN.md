# Implementation plan: vk/5f70-not-loading-chat

See `SPEC.md` (R1–R5) and `specs/vk/5f70-not-loading-chat/plan.md`.
Frontend only, in `packages/web-core`. Paths below are relative to
`packages/web-core/src/`.

## Steps

1. **Constant.** Add `HISTORY_STREAM_IDLE_TIMEOUT_MS = 30_000` to
   `shared/hooks/useConversationHistory/constants.ts`, with a rationale:
   reset per message, and live streams are exempt.
2. **Settle-once stream.** In `shared/lib/streamJsonPatchEntries.ts`:
   - add `idleTimeoutMs?` to `StreamOptions` and document `onError`'s
     terminal cases;
   - add `settled` and `idleTimer` state, plus `clearIdleTimer` / `fail` /
     `armIdleTimer` helpers. `fail` is a no-op once settled or caller-closed,
     and otherwise closes the socket and calls `onError` once;
   - arm the timer when the stream is created (this covers a handshake that
     never completes) and re-arm it on every message. Ignore messages after
     settlement;
   - the `finished` path sets `settled` and clears the timer before
     `onFinished`;
   - route the transport `error` event, parse errors, and open failures to
     `fail`. The `close` listener calls `fail` ("closed before finished");
   - when the open resolves after settlement, close the new socket at once;
   - `close()` clears the timer.
3. **Wire.** In `features/workspace-chat/model/hooks/useConversationHistory.ts`,
   pass `idleTimeoutMs: HISTORY_STREAM_IDLE_TIMEOUT_MS` only from
   `loadEntriesForHistoricExecutionProcess`. `loadRunningAndEmit` stays
   unbounded, and its existing `onError → reject → backoff` now also covers a
   close without `finished`.
4. **Tests.** Add `shared/lib/streamJsonPatchEntries.test.ts` (jsdom, mocked
   `openLocalApiWebSocket`, fake timers, an event-listener `FakeSocket`), with
   8 cases, one per terminal path, each asserting a single settlement. Run it
   against the pre-fix file to confirm the new-behaviour cases fail there.
5. **Verify.** Run the web-core vitest suite, `tsc` for
   local-web/remote-web/web-core/ui, the frontend lint, the i18n check, and
   prettier. No Rust changed, so the cargo checks are not needed.
6. **Review.** Iterate an independent Codex review until clean.
7. **Knowledge.** Add a wiki page for the stream settlement contract and the
   debugging recipe (coordinator direct access, phone-width iframe
   reproduction, and reading `/api/cluster/metrics` for I/O wait).
8. **Ship.** Commit, open a PR against `main`, and merge.

## Rollback

Revert the single commit. There is no data, schema, or protocol change.
