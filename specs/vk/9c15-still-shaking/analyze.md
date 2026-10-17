# `/speckit.analyze`: Stable Earlier-History Loading Geometry

## Findings

1. **[info] `spec.md` / `research.md`** consistently distinguish this defect
   from PR #268: the remaining motion is the variable-height earlier-history
   loading control, not repeated plan navigation.
2. **[info] `plan.md` / `tasks.md`** map every production change to the shared
   `web-core` conversation surface and introduce no backend, generated type,
   dependency, deployment, or other-service work.
3. **[info] `tasks.md`** establishes a fail-before-fix regression and orders the
   component implementation before parent integration.
4. **[info] `contracts/earlier-history-control.md` / `tasks.md`** cover idle,
   loading, and error/retry presentation. The absent state remains a parent
   conditional and needs no geometry assertion because it intentionally owns no
   space.
5. **[info] Constitution XXXVI** is satisfied: presentation state ceases to act
   as an uncoordinated scroll/layout authority, while the existing semantic
   anchor remains responsible for real history insertion.
6. **[info] SpecKit prompt paths** are stale and refer to an unrelated completed
   feature. Current artifacts correctly use the task-owned directory and do not
   modify that unrelated task.

## Result

No errors, warnings, coverage gaps, or constitution violations remain. The task
set is ready for implementation.
