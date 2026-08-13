# Implementation Plan: Authoritative Execution Status Reconciliation

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical Context

- Backend: Rust 2024, Tokio broadcast streams, SQLx/SQLite, Axum WebSockets in
  `crates/services` and `crates/server`.
- Frontend: React/TypeScript and the shared `useJsonPatchWsStream` hook in
  `packages/web-core`.
- Existing contract: a session execution WebSocket emits a full
  `replace /execution_processes` snapshot, then `Ready`, then incremental
  patches. The hook intentionally retains the last snapshot across reconnects.
- Status domain: only `running` is active; all five other statuses clear Stop.
- Scope: Vibe Kanban only, no new dependency, no generated contract change.

## Architecture & Approach

### 1. Close the stream initialization race

Refactor
`EventService::stream_execution_processes_for_session_raw` in
`crates/services/src/services/events/streams.rs` so the broadcast receiver is
subscribed before the awaited database snapshot query. Build the filtered live
stream from that already-subscribed receiver, then chain the authoritative
snapshot and `Ready` ahead of buffered/live events.

This makes the handoff gap lossless: a terminal update committed while the
snapshot is being loaded is buffered for the new subscriber and applied after
the snapshot. Reconnects therefore converge even when completion occurs during
stream initialization.

### 2. Add a deterministic regression seam

Extract or parameterize the snapshot-plus-subscribed-stream construction just
enough for a service test to pause after subscription and before snapshot
completion. The test will publish a running-to-terminal update in that window,
consume the initial snapshot/Ready/update sequence, and prove the final reduced
state is terminal. A companion assertion retains an active `running` process.

Prefer testing the event service contract directly over timing a real socket;
the socket route in `crates/server/src/routes/execution_processes.rs` is a
transparent forwarding layer.

### 3. Lock the frontend reconnect contract

Extend
`packages/web-core/src/shared/hooks/useJsonPatchWsStream.reconnect.test.tsx`
to simulate a missed terminal update, unexpected close, and reconnect. The
second connection sends a full replacement snapshot followed by `Ready`; assert
the retained stale running value becomes terminal. This covers the shared
client behavior that drives `useExecutionProcesses`, while existing exact
`status === running` derivation in `useExecutionProcesses.ts` and
`SessionChatBoxContainer.tsx` preserves Stop for active work.

### 4. Verify restart finalization

Run the focused orphan-cleanup/shutdown tests around
`ContainerService::cleanup_orphan_executions` and local deployment shutdown.
Only add backend code if these tests expose an execution that can remain
`running` without positive worker/process evidence; otherwise document the
existing `interrupted`/`indeterminate` recovery as verified rather than
duplicating lifecycle logic.

## Data Model

See `./data-model.md`. No schema or generated type changes are planned.

## Contracts

See `./contracts/execution-process-stream.md`.

## Research Notes

See `./research.md`. No new dependency is introduced.

## Constitution Check

- Principle II: service and rendered-hook regression tests cover missed
  terminal events and the active Stop case.
- Principles VI and XII: the fix strengthens the existing snapshot/broadcast
  handoff rather than adding a second execution-status channel.
- Principles XVIII and XXX: only authoritative backend evidence changes
  lifecycle state, while every reconnect rehydrates from a full snapshot.
- Principle XIX: patch streaming remains an optimization over full snapshots.

No constitution deviation remains.

## Risks & Dependencies

- Subscribe-before-query can yield a duplicate update already reflected in the
  snapshot. JSON Patch add/replace application must remain idempotent for the
  keyed process value; the existing upsert patch semantics provide this.
- Broadcast lag must not silently look authoritative. Existing lag handling
  ends the stream, causing the client to reconnect and resnapshot.
- Frontend tests alone cannot prove the backend handoff is lossless; both sides
  require focused coverage.
- Historical artifacts already present beside the command-selected spec path
  are unrelated and must not be treated as implementation scope.
