# Clarifications: Compact Right Drawer Section Spacing

## Resolved Decisions

1. **Which section changes?** Server Affinity is the only currently identified
   compact expanded section and is the only section whose sizing behavior
   changes in this task.
2. **What happens to other sections?** Every other existing drawer section keeps
   its current fill-available-space behavior. The composition should express
   the decision per section so future compact sections can opt out deliberately.
3. **Should the Server Affinity row layout change?** No. Project knowledge and
   the screenshot show that its compact two-column grid is correct; the parent
   section's flex growth creates the gap.
4. **Should the shared disclosure state model change?** No. The primitive
   continues to own expansion and persistence; the caller supplies only the
   section's sizing policy.
5. **Are fixed heights acceptable?** No. Server Affinity uses intrinsic height,
   and content panels keep the existing bounded flex/overflow chain.

## Open Questions

None.
