# Tasks: The chat panel always finishes loading

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes. Paths are relative to `packages/web-core/src/`.

## Phase 1: Setup
- [x] T001 [P] Add `HISTORY_STREAM_IDLE_TIMEOUT_MS = 30_000` with a rationale comment in `shared/hooks/useConversationHistory/constants.ts`

## Phase 2: Core
- [x] T002 [P] Make `streamJsonPatchEntries` settle exactly once: add a `settled` guard with `finish`/`fail` helpers, fail on close without `finished` unless the caller closed it, and add the optional `idleTimeoutMs` (armed at start, reset per message, closes the socket and fails on expiry). File: `shared/lib/streamJsonPatchEntries.ts`
- [x] T003 Pass `idleTimeoutMs: HISTORY_STREAM_IDLE_TIMEOUT_MS` from `loadEntriesForHistoricExecutionProcess`, leaving `loadRunningAndEmit` unbounded, in `features/workspace-chat/model/hooks/useConversationHistory.ts` (depends on T001, T002)

## Phase 3: Validation
- [x] T004 [P] Add `shared/lib/streamJsonPatchEntries.test.ts` covering: finished → single `onFinished`; clean close without finished → single `onError`; error+close → single `onError`; idle expiry → `onError` and socket closed; steady messages → no idle error; caller `close()` → silent; no `idleTimeoutMs` → silence never errors (depends on T002)
- [x] T005 Run `pnpm --filter @vibe/web-core test`, `pnpm run check`, `pnpm run lint`, and `pnpm run format` (depends on T003, T004)
