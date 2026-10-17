# Research: Stable Earlier-History Loading Geometry

## Recording Evidence

The supplied recording is 7.44 seconds at 3160×2034. A 4-fps full-screen contact
sheet and a 10-fps two-second crop show:

- workspace chrome and the composer remain fixed;
- the conversation moves down when the centered “Loading earlier messages”
  skeleton/label appears;
- it moves back up when that state is replaced;
- the cycle repeats while automatic earlier pages load.

This is distinct from PR #268's much larger plan-reveal/bottom-follow
oscillation.

## Code-Path Finding

`ConversationListContainer.requestEarlierHistory` captures the first visible
semantic row and awaits `loadEarlier()`. The correction loop begins only after
that promise resolves. During the await, `isLoadingEarlier` changes the control
above the rows from a compact button to two skeleton lines plus a label. Because
both are normal-flow content with different heights, visible rows move before
anchor correction is active. The sentinel can immediately request another page,
repeating the transition.

## Decision

Keep the earlier-history control's block geometry invariant across its idle and
loading states. This removes the causal layout delta while preserving the
existing semantic correction for actual page insertion.

## Alternatives Rejected

- Start a correction animation loop before awaiting the request: request time is
  unbounded, so this either spins indefinitely or needs arbitrary deadlines.
- Hide loading feedback: avoids the delta but removes useful progress state.
- Disable automatic pagination: changes product behavior and makes older history
  less accessible.
- Change virtualized-tail partitioning: frame evidence points to the visible
  history control, and this would widen performance and scroll risk.
- Apply a global fixed header/overlay: unnecessarily changes scroll ownership and
  accessibility order.

## Dependencies

None added.
