# `/speckit.analyze`: Stable Streaming Chat Viewport

## Initial findings

1. **[error] `spec.md` acceptance criterion 1** still described a transient
   virtualized-tail boundary after clarification established repeated plan
   reveal as the root cause. The regression criterion must name the actual
   edge-trigger contract.
2. **[warning] `tasks.md` T001** placed entry-aware plan detection in the generic
   scroll-command module, which would couple scroll policy to normalized entry
   shapes. A focused helper beside `useConversationHistory` is a clearer
   boundary.
3. **[warning] `tasks.md` T002** did not explicitly include scope-change reset
   coverage, despite the plan naming cross-conversation suppression as a risk.
4. **[info] `tasks.md` T003** proposed removing duplicate sentinel markup, but
   inspection shows exactly one rendered sentinel. The apparent duplicate came
   from overlapping diagnostic output and must not become an unrelated edit.
5. **[info] SpecKit prompt paths** refer to an unrelated existing feature. All
   current artifacts correctly use the task-owned directory and preserve the
   unrelated files.

## Resolution

The spec, plan, and tasks were corrected for findings 1–4 before implementation.
The resulting change is minimal, testable, shared across both frontends, and
compliant with constitution principles II, III, IV, VI, XIV, and XXXVI. No
significant gaps or constitution violations remain.
