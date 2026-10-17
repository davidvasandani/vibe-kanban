# `/speckit.clarify`: Stable Streaming Chat Viewport

## Resolved questions

1. **What causes the two-position oscillation in the recording?**
   `useConversationHistory.emitEntries` rewrites every update to `addType =
   'plan'` whenever the latest normalized entry is `ExitPlanMode`. That condition
   stays true across subsequent snapshots. Each update therefore reissues
   plan-reveal (align the last row to the viewport top), while existing live-tail
   correction pulls the scroller back to the bottom. The two valid mechanisms
   become competing scroll authorities and alternate positions.
2. **Should streaming/tail-boundary derivation change?** No. The evidence does
   not implicate transient loading rows or running-process derivation. Changing
   them would widen the fix and risk the intentionally unvirtualized live tail.
3. **When should plan reveal run?** Exactly once when a new logical
   `ExitPlanMode` entry becomes the latest entry. Re-emissions of the same entry
   are normal running updates. A later distinct plan-exit entry must still reveal
   once.
4. **How should identity be tracked?** Use the existing stable `patchKey`, scoped
   to the conversation-hook lifecycle. Do not derive identity from the array
   index or displayed text.

## Remaining open questions

None.
