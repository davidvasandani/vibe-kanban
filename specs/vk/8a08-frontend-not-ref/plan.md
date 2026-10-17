# Implementation Plan: Show Sent Chat Messages Without Refreshing

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical Context

The affected surface is the shared React/TypeScript frontend in
`packages/web-core`, used by local and remote frontends. Existing-session sends
flow from `SessionChatBoxContainer.tsx` through `useSessionSend.ts` to
`sessionsApi.followUp`, which already returns a complete server-created
`ExecutionProcess`. The hook currently discards that value and returns a
boolean.

Conversation history consumes `executionProcessesVisible` from
`ExecutionProcessesProvider`. That provider currently projects only
`useExecutionProcesses`, whose state is populated by a JSON Patch WebSocket
snapshot and later patches. A missed creation patch therefore leaves no process
for `useConversationHistory` to discover until a refresh obtains another
snapshot.

No backend, database, generated type, dependency, deployment, or other service
change is required.

## Architecture & Approach

1. Add provider-owned response reconciliation to
   `packages/web-core/src/shared/providers/ExecutionProcessesProvider.tsx`.
   Store successful response processes in an ID-keyed map scoped to the
   provider's `sessionId`.
2. Merge that map with streamed processes by ID, with streamed values applied
   last. The resulting existing context arrays/maps feed conversation history,
   running-state derivation, and process-list consumers without a second
   conversation representation.
3. Expose `reconcileExecutionProcess(process)` on
   `ExecutionProcessesContextType`. Reject mismatched session IDs and clear the
   bridge map when `sessionId` changes.
4. When the live stream contains a bridged ID, remove that bridge entry after
   render. The keyed merged projection remains stable and the live stream owns
   all later replacements/removals.
5. Extend `useSessionSend` with an optional
   `onFollowUpAccepted(ExecutionProcess)` callback. Invoke it synchronously
   after `sessionsApi.followUp` resolves and before returning success.
6. In `SessionChatBoxContainer`, obtain the reconciliation function from the
   existing execution-process context and pass it into `useSessionSend`.
   Composer cleanup stays gated by the existing true result.
7. Add focused provider tests for response-first, stream-first, mismatch,
   session reset, and stream supersession behavior. Add hook tests for
   success-only callback delivery and failure preservation.

## Data Model

See `./data-model.md`. This feature adds ephemeral client reconciliation state
only; durable models and generated contracts are unchanged.

## Contracts

See `./contracts/follow-up-process-reconciliation.md` for the internal shared
frontend handoff contract. Existing HTTP and WebSocket wire contracts are
unchanged.

## Research Notes

See `./research.md`. No new dependency is required.

## Constitution Check

- Principle II: provider and send-hook tests cover both race orderings, failure,
  and session isolation.
- Principles III and VI: the smallest fix reuses the server-returned process and
  existing execution projection; it does not create a synthetic message layer.
- Principle IV: all behavior lives in shared `web-core`.
- Principle XII: the HTTP-to-WebSocket handoff is keyed and idempotent across
  both orderings.
- Principle XXX: the full snapshot/patch stream remains authoritative and
  supersedes bridge state.
- Principle XXXV: reconciliation requires matching session identity.
- Constraint XIV: locked dependencies and repository verification are run
  before completion.

No constitution deviation or unresolved question remains.

## Risks & Dependencies

- Removing bridge state too early can recreate the gap. It is removed only after
  the streamed projection contains the same stable process ID.
- Keeping it after live ownership begins can resurrect a process removed by a
  later stream patch. Cleanup and a regression test cover that transition.
- A response can settle after the user switches sessions. The provider callback
  validates the process against its current `sessionId`, and session changes
  reset bridge state.
- React callback identity and effect timing can cause unnecessary rerenders.
  Stable callbacks and functional state updates avoid stale closures and
  duplicate insertion.
