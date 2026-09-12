# `/speckit.clarify`: Stable Streaming Conversation Viewport

## Resolved Questions

1. **What is moving in the recording?** The conversation moves down when the
   taller “Loading earlier messages” skeleton appears and back up when it is
   replaced. Surrounding chrome is fixed. This matches the normal-flow history
   control in `ConversationListContainer`, not a virtualized-tail ownership
   change.
2. **What should own loading-state stability?** The history control's layout
   contract. Its idle and loading presentations must reserve identical vertical
   space, so semantic anchor correction deals only with inserted history rows.
3. **What regression level is required?** Pure deterministic coverage is the
   primary gate for the partition state machine because JSDOM has no real
   layout. Existing scroll-intent tests protect policy selection. The supplied
   recording and manual/browser verification provide pixel-level evidence; a
   new browser harness is not required for this focused correction.
4. **Is this the same defect as PR #268?** No. PR #268 removed repeated semantic
   plan navigation. The new recording shows smaller continuous live-tail motion.
   Plan-reveal state and batching remain unchanged unless a regression exposes a
   direct interaction.

## Remaining Open Questions

None.
