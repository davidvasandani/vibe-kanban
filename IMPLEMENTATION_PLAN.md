# Implementation Plan: Preserve Preview App Navigation URLs

1. Trace the current preview URL lifecycle from dev-server detection through
   proxy URL construction, iframe navigation reporting, and preview scratch
   settings restoration.
2. Establish the persistence semantics that distinguish the latest navigated app
   URL from an explicit user override, including local/proxy URL normalization.
3. Extend the smallest existing preview state boundary needed to save and restore
   the latest accepted navigation URL per workspace.
4. Prevent duplicate writes, stale bridge messages, workspace cross-talk, and
   leakage of preview-only query parameters.
5. Add focused automated tests for route components (path/query/hash), remount
   restoration, workspace isolation, and override compatibility.
6. Run focused tests, frontend checks, formatting, and linting; repair any
   regressions.
7. Independently review the complete diff, address significant findings, and
   repeat verification.
8. Document reusable preview persistence invariants in the project knowledge
   base and refresh its index.
9. Commit the implementation, open a pull request against the base branch, wait
   for required checks, and merge it.
