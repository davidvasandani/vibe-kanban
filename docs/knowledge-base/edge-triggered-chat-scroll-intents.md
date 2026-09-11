# Edge-triggered chat scroll intents

Tags: `vk/9ba3-stop-the-chat-fr`

## Snapshot facts are not lifecycle events

Workspace conversation updates carry full current snapshots. A condition that
is true in a snapshot—such as “the latest normalized entry is `ExitPlanMode`”—
can remain true across many stream callbacks. Converting that condition directly
into a navigation event makes the event level-triggered: every callback issues
the same scroll command again.

That is especially dangerous when another mechanism owns the steady-state
viewport. In this case repeated plan reveal aligned content to the viewport top
while bottom lock aligned it to the live tail, creating a visible two-position
oscillation.

Turn a persistent snapshot fact into an edge by remembering the stable semantic
identity already attached to the entry (`patchKey`). Emit the special intent
only when that identity differs from the last handled identity. Reset the memory
at the same scope boundary that resets the conversation generation.

## One-shot events must survive batching

Edge-triggering upstream is insufficient when downstream rendering coalesces
updates. If a one-shot `plan` update is followed by an ordinary `running` update
before the next animation frame, newest-snapshot-wins batching can erase the only
special event before it renders.

Coalescing therefore has two independent rules:

- newest snapshot data wins;
- an unconsumed one-shot semantic annotation survives until that batch is
  claimed.

Clear the pending batch synchronously when the consumer claims it. Leaving the
processed value in a “pending” ref turns the preserved one-shot annotation back
into a sticky level-trigger and recreates the original bug.

## Regression pattern

Keep the transition and coalescing rules pure and cover:

1. first observation of identity A emits the special intent;
2. repeated observation of A preserves the ordinary update type;
3. distinct identity B emits once;
4. scope reset lets A emit in a new conversation;
5. `plan` followed by `running` in one render batch remains `plan`;
6. after the batch is consumed, later `running` updates remain `running`.

The last rule crosses the pure helper and its ref lifecycle, so independent diff
review is valuable even when helper-level tests pass; it caught both “event
erased before render” and “consumed event never cleared” during this task.
