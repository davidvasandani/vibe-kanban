# Clarifications: Stable Workspace Order During Restart

## Resolved Decisions

1. **How long does the base `updated_at` fallback apply?** Permanently whenever
   no valid latest-process completion timestamp exists. The UI cannot reliably
   distinguish “summary still loading” from “summary loaded and this workspace
   has never completed a process” using the current value shape. The persisted
   base timestamp is meaningful in both states and avoids reintroducing missing
   values after the query settles.
2. **What is the final tie-breaker?** Workspace ID after workspace name. IDs are
   stable and unique, so this makes exact timestamp/name ties deterministic
   without adding a new product concept.
3. **Should missing values change position based on direction?** No. Missing or
   invalid selected timestamps always sort after valid timestamps. Ascending or
   descending only controls comparisons between valid timestamps.
4. **Is an API or schema change required?** No. The streamed workspace record
   already supplies persisted `updated_at`, and the existing sidebar model
   retains it as `updatedAt`.

## Remaining Open Questions

None.
