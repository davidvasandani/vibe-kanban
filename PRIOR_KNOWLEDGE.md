# Prior Knowledge: Streaming Conversation Viewport Stability

Task: `vk/9c15-still-shaking`

Searched `docs/knowledge-base/`, its index, and the legacy `wiki/` for
conversation scrolling, viewport anchoring, virtualization, streaming, and
layout ownership.

## Directly Relevant Knowledge

### `docs/knowledge-base/edge-triggered-chat-scroll-intents.md`

- Conversation callbacks contain snapshots, not lifecycle events. A persistent
  snapshot fact must be converted to a one-shot intent using stable semantic
  identity.
- PR #268 applied this to `ExitPlanMode`: remember the handled `patchKey`, carry
  the one-shot `plan` annotation through animation-frame batching, and clear the
  batch synchronously when claimed.
- This fixed the large oscillation between plan alignment and bottom alignment.
  The new recording shows smaller continuous motion, so the next fix must not
  undo or duplicate that event-boundary work.
- Regression coverage should cross pure state rules and the consuming ref /
  render lifecycle because a correct helper can still become sticky in its
  caller.

### `docs/knowledge-base/lazy-loading-normalized-conversation-history.md`

- Conversation history is a bounded recent window with explicit earlier-page
  loading and release. Stable semantic keys are required to preserve the
  reader's visible anchor across prepend/release transitions.
- Live revisions must remain authoritative while historical materialization is
  in flight. A viewport fix must not conflate history-window mutation with
  streaming-tail mutation.

### `docs/knowledge-base/nested-flex-scroll-containment.md`

- A scroll surface should have one clear owner. Competing containment or
  measurement mechanisms create unstable behavior and broaden the blast radius.
- JSDOM does not calculate browser layout. Pure state tests and rendered
  structural assertions are deterministic, but pixel-level behavior still
  requires browser/manual evidence.

### `docs/knowledge-base/prompt-driven-agent-pipelines.md`

- Pipeline stages and named artifacts are an executable contract. Follow the
  supplied stage order literally and keep task-scoped SpecKit artifacts under
  `specs/vk/<task-id>/`.

## Adjacent Knowledge

- `wiki/workspace-carousel-view.md` records that each mounted conversation owns
  its vertical scroller and that unnecessary remounting loses scroll state.
- `docs/knowledge-base/authoritative-snapshot-stream-handoffs.md` reinforces
  that retained state should reset only when its logical stream scope changes,
  not on ordinary patch snapshots.
- The knowledge base does not yet document a durable rule for partitioning a
  conversation between a virtualized head and an unvirtualized live tail. That
  is a stage-12 candidate if the implementation confirms a reusable invariant.

## Consequences for the Spec and Plan

1. Treat PR #268's edge-triggered plan reveal as a preserved invariant.
2. Model the virtualized/tail split as lifecycle state with an explicit scope
   reset, rather than deriving ownership anew from every streaming snapshot.
3. Ensure one mechanism owns bottom following; measurement compensation must
   not fight it while bottom lock is active.
4. Test active/settled transitions and scope resets with pure deterministic
   helpers, then run focused frontend checks and inspect browser-visible motion.
5. Keep all changes in the Vibe Kanban repository; homelab deployment is out of
   scope.
