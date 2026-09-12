# Technical Plan: Stable Earlier-History Loading Geometry

**Spec**: `./spec.md`
**Status**: Ready for task decomposition

## Technical Context

The defect is in the shared React/TypeScript workspace conversation feature at
`packages/web-core/src/features/workspace-chat/ui/ConversationListContainer.tsx`.
Earlier-history pagination is asynchronous; an `IntersectionObserver` triggers
`requestEarlierHistory`, which captures a semantic anchor, awaits the page, and
then starts correction. The load control above the conversation is normal-flow
DOM and changes height during that await.

No dependency, backend API, persisted model, generated type, deployment, or
other-service change is required.

## Architecture & Approach

1. Extract the earlier-history status/control presentation from
   `ConversationListContainer.tsx` into a small local shared-feature component.
2. Overlay the idle and loading presentations in the same grid cell while both
   participate in intrinsic sizing. The larger presentation determines one
   shared responsive height even when translations wrap or text is enlarged.
3. Keep the component conditional on history availability/loading/error, so no
   permanent gap is added when the conversation has no earlier-history state.
4. Preserve `requestEarlierHistory` and its semantic anchor correction exactly;
   it remains responsible for actual inserted/removed history geometry after the
   request commits.
5. Add a rendered-DOM Vitest that renders idle and loading states and asserts the
   same geometry-owning element and invariant sizing contract in both. Cover the
   retry/error affordance without coupling the test to browser pixel layout.
6. Re-run existing paging, plan-reveal, scroll-command, and web-core verification
   to protect adjacent behavior.

## Data Model

See `./data-model.md`. No persisted data changes.

## Contracts

See `./contracts/earlier-history-control.md`. No network contract changes.

## Research Notes

See `./research.md`.

## Constitution Check

- II (test the contract): a rendered-DOM test protects invariant control
  geometry across state changes.
- III (small, reversible steps): the fix changes only the presentation whose
  normal-flow height causes the motion.
- IV (shared boundaries): the component stays inside shared `web-core`, covering
  local and remote shells.
- VI (don't rebuild what shipped): existing paging and semantic anchoring remain
  intact.
- XXXVI (one scroll authority): loading presentation no longer moves content
  before the semantic anchor policy can act.

No deviations or open constitution questions remain.

## Risks & Dependencies

- Removing the inactive layer from layout would reintroduce the height delta.
  Keep it `invisible` rather than conditionally absent or `display: none`.
- JSDOM cannot measure pixels. The test protects the shared sizing contract;
  recording/manual evidence validates the browser-visible effect.
