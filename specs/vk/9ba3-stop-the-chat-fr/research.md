# Research: Stable Streaming Chat Viewport

## Recording evidence

The supplied recording is 13.38 seconds at 3160×2034 and 120 fps. Sampling at
two frames per second shows the message composer and workspace side panels fixed
while the conversation repeatedly alternates between two vertical offsets. One
offset aligns recent plan/follow-up content nearer the top; the other follows
the conversation bottom.

## Code-path finding

`useConversationHistory.emitEntries` inspects the last entry in every full
snapshot. If it is the stable `ExitPlanMode` tool-use entry, it changes the
snapshot's update type to `plan`. Because that entry remains last until another
normalized entry arrives, every stream callback repeats `plan`.

`resolveScrollIntent('plan', ..., true)` creates `plan-reveal`, which aligns the
last row to the viewport top and may grow the plan spacer. Separately,
`scrollToBottom` activates bottom lock, whose layout effect corrects to maximum
scroll as counts/sizes update. Repeated `plan` classification therefore makes
two mechanisms legitimately request opposite offsets.

## Decision

Make plan detection edge-triggered by existing `patchKey`. A first observation
reveals the plan; identical later observations preserve their original update
type. This fixes the cause at the semantic boundary rather than adding timeout
suppression to either scroll mechanism.

## Alternatives rejected

- Disable bottom locking after plan reveal: this would stop live-tail following
  after the agent continues and violates expected streaming behavior.
- Remove plan reveal: this regresses the intentional plan presentation feature.
- Add a timing cooldown: stream cadence and plan duration are unbounded, so a
  cooldown merely changes the oscillation frequency.
- Change the virtualized tail boundary: recording and code trace identify no
  boundary oscillation; changing it risks large-transcript performance.

## Dependencies

None added.
