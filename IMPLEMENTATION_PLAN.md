# Implementation Plan: Stop Workspace Chat Viewport Shaking

1. Reconstruct the failure from the recording and current conversation-list
   code, tracing live timeline updates through row derivation, the
   virtualized/unvirtualized tail boundary, TanStack measurement, at-bottom
   detection, and bottom-lock correction.
2. Run the required SpecKit constitution, specification, clarification,
   planning, task decomposition, and analysis stages. Record the exact
   invariant responsible for the oscillation before changing product code.
3. Extract the smallest pure state transition or scroll-policy seam needed for
   deterministic tests. Cover continuous streaming at the bottom, streaming
   after reader scroll-up, and transitions between active and settled tails.
4. Implement the minimal conversation-list/virtualizer change that makes tail
   ownership and bottom correction stable across live updates, without changing
   semantic row identity, history-prepend anchoring, interaction anchoring, or
   programmatic navigation.
5. Run focused regression tests first, then install dependencies if required and
   run repository formatting, frontend type checks, linting, and relevant test
   suites. Perform a visual/browser reproduction when the local fixture path can
   exercise the affected transcript.
6. Run an independent Codex CLI review of the diff. Address every confirmed
   significant finding and repeat verification/review until clear.
7. Distill reusable scroll/virtualization guidance into the Vibe Kanban
   knowledge base, update its index and task tag, and commit those docs.
8. Commit the implementation, push the task branch, open a pull request against
   the repository's base branch, monitor required checks, resolve failures, and
   merge the pull request.

## Guardrails

- Do not change another service or homelab deployment configuration.
- Do not replace the single conversation scroller or remove bounded history.
- Do not rely on timing-only suppression as the primary correctness mechanism.
- Preserve user-controlled scroll position and accessibility of existing
  navigation/load controls.
