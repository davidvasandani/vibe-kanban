# Edge-triggered chat scroll intents

Tags: `vk/9ba3-stop-the-chat-fr`, `vk/9c15-still-shaking`

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

## Async status UI must not become a layout authority

Fixing repeated semantic navigation can expose a second source of motion. The
earlier-history loader sits in normal flow above conversation rows. Its idle
button was shorter than its loading skeleton and label, while semantic anchor
correction began only after the page request completed. Each automatic page
therefore moved visible rows down during loading and back after correction.

Keep asynchronous status presentations geometrically invariant when they sit
inside a scroll surface. For responsive text and translations, avoid relying on
a guessed `min-height`: overlay hidden, `aria-hidden`, noninteractive sizing
copies of every state in one grid cell so the largest intrinsic state reserves
space continuously. Render the active interactive state separately so hiding a
focused button cannot leave focus inside an inaccessible element. Reserve error
feedback geometry too, because retry commonly clears the error while entering
loading.

Sizing copies must be inert in performance as well as interaction. Do not run
invisible pulse animations; apply animation only to the active loading state.
Rendered-DOM coverage can assert shared grid ownership, persistent sizing
layers, active-button removal during loading, error-row reservation, and the
absence of animation on invisible copies. Browser evidence remains necessary
for the pixel-level outcome because JSDOM does not calculate layout.
