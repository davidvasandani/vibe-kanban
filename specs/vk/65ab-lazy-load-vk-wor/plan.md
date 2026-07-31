# Implementation Plan: Lazy-load workspace chat history

**Spec**: `./spec.md`
**Status**: Ready for task breakdown

## Technical context

- Backend: Rust, Axum, Tokio, SQLx/SQLite, JSON Patch, `MsgStore`.
- Frontend: React/TypeScript in shared `packages/web-core`, TanStack Virtual,
  signed WebSocket helpers.
- Raw execution logs are durable; normalized entries currently exist only as an
  in-memory/live or reconstructed patch stream.
- Shared web-core changes affect local and remote frontends.

## Architecture and approach

1. Add a durable normalized-transcript materialization beside execution log
   storage in `crates/services/src/services/execution_process.rs` and
   `crates/services/src/services/container.rs`. Apply add/replace/remove patches
   by stable process/index identity and track revision/completion atomically.
2. Feed live normalizer patches into that materialization from the existing
   execution persistence boundary. For legacy processes, add a service-owned
   rollout queue that reuses the cancellable, semaphore-bounded historical
   normalizer and marks page readiness outside interactive requests.
3. Add session-level tail/older page queries in the service layer and expose
   them from a session-authorized Axum route. Enforce limit caps, deterministic
   process/id ordering, cursor scope/generation validation, and structured
   exhaustion/errors.
4. Add revision watermarks/resume behavior to the normalized live channel so a
   tail page hands off without a gap. Keep raw script streaming unchanged.
5. Refactor
   `packages/web-core/src/features/workspace-chat/model/hooks/useConversationHistory.ts`
   to request one recent session page, preserve absolute stable keys, open live
   streams only for active processes, and expose single-flight
   `loadEarlier`/retry/exhaustion state. Delete idle background preload.
6. In
   `packages/web-core/src/features/workspace-chat/ui/ConversationListContainer.tsx`,
   add a top sentinel and accessible action. Capture the first visible semantic
   row/offset before paging and correct scroll after the prepended rows render
   and measure.
7. Regenerate shared API types from Rust if response structs are exported; do
   not hand-edit `shared/types.ts`.
8. Test materialization correctness, legacy build cancellation, paging/cursors,
   snapshot/live race, frontend single-flight/stale scope behavior, and scroll
   anchoring. Format and run focused then repository-wide checks.

## Data model

See `./data-model.md`.

## Contracts

See `./contracts/history-api.md`.

## Research notes

See `./research.md`.

## Constitution check

- I/III/VI: extends the existing normalization, `MsgStore`, signed routes, and
  shared conversation derivation rather than creating a parallel chat UI.
- II: contracts define deterministic backend and rendered frontend tests before
  implementation.
- IV: data/feature behavior remains in `web-core`; UI primitives are reused.
- IX: vendor normalization remains defensive and final add/replace/remove
  identity is preserved.
- XII/XVII: snapshot/live ownership uses an explicit process revision boundary.
- XIV: verification uses repository commands and generated types are regenerated.
- XVIII: history is server-bounded, demand-driven, resumable, scope-cancelled,
  and scroll-anchor preserving.

No constitution deviations or open questions remain.

## Risks and dependencies

- Legacy rollout pays one full cancellable normalization to create materialized
  state. Readiness/preparing state must be explicit in telemetry and UI, and
  rollout must never block interactive histories beyond its bounded permit pool.
- Materializer corruption could change displayed history; raw logs remain the
  source of truth and fingerprint/schema invalidation permits rebuild.
- Patch revisions must cover every producer sharing the process `MsgStore`.
- Remote authorization/routing must be verified before sharing the new route.
- No new external dependency is planned.
