# Technical Plan: Stable Streaming Chat Viewport

**Spec**: `./spec.md`
**Status**: Ready for task decomposition

## Technical Context

The affected shared React/TypeScript feature is in
`packages/web-core/src/features/workspace-chat`. Timeline updates are snapshots
annotated with an `AddEntryType`; `useScrollCommandExecutor` maps that annotation
to an imperative scroll action. TanStack Virtual owns the historical head while
recent/live rows remain normal DOM nodes.

No dependency, API, persistence, or deployment change is required.

## Architecture & Approach

1. Add a pure plan-reveal transition helper beside conversation-history logic.
   It accepts the latest stable entry identity and the identity most recently
   revealed, and returns both the effective `AddEntryType` and next remembered
   identity.
2. In `useConversationHistory`, retain the last revealed plan entry in a ref
   scoped to the mounted conversation lifecycle. Reset it when the hook's scope
   changes. Feed each emitted snapshot through the helper and test the reset
   policy independently.
3. Classify a newly observed `ExitPlanMode` patch as `plan`; classify repeated
   snapshots ending in the same patch as their original update type (`running`
   or `historic`). This removes the second scroll authority while preserving the
   first reveal and later distinct plans.
4. Add unit tests covering first observation, repeated observation, no plan,
   and a later distinct plan. Retain existing scroll-intent behavior unchanged.

## Data Model

See `./data-model.md`. The only new state is ephemeral client lifecycle state.

## Contracts

See `./contracts/plan-reveal-transition.md`. There is no network contract change.

## Research Notes

See `./research.md`.

## Constitution Check

- II (test the contract): the edge-trigger transition is pure and directly
  unit-tested.
- III (small, reversible steps): no virtualizer, layout, or backend rewrite.
- IV (shared boundaries): the fix remains in shared `web-core`, benefiting local
  and remote frontends.
- VI (don't rebuild what shipped): existing stable `patchKey` identity and
  scroll-intent machinery are reused.
- XXXVI (one scroll authority): repeated snapshots cannot continually reactivate
  plan navigation against live-tail following.

No deviations or open constitution questions remain.

## Risks & Dependencies

- A reset at the wrong granularity could suppress the first plan of another
  conversation. Mitigation: reset remembered identity on conversation scope.
- Remembering only one identity means a malformed stream returning to an older
  plan could reveal it again after a newer plan. Normalized entries are append-
  ordered; the contract intentionally tracks the most recently revealed edge.
- Existing plan-reveal behavior for a genuinely new plan must remain covered.
