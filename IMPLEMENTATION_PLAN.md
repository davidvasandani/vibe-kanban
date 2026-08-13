# Implementation plan: Move Refresh

1. Inspect the existing AppBar deployment/update branches, workspace layout
   data flow, right-sidebar section primitives, persistence keys, and relevant
   rendered-DOM tests.
2. Preserve the AppBar's native `Update` path while removing its deployed git
   revision and web-deployment `Refresh` fallback.
3. Thread the existing `deployUpdateAvailable` state and page reload callback
   from the workspace layout into `RightSidebar` without changing backend or
   detection contracts.
4. Convert Deploy Status from a fixed row to the shared collapsible section
   model, add a persisted expansion key if required, retain compact revision/age
   metadata in the header, and add a conditional Refresh section action whose
   click does not toggle the accordion.
5. Update tests to prove the Deploy Status accordion placement and metadata,
   conditional action visibility/callback behavior, and the AppBar ownership
   change while protecting native Update behavior.
6. Run focused tests, frontend type/lint checks, repository formatting, and
   authored-file diff checks; resolve any regressions.
7. Run the required independent Codex diff review, address confirmed findings,
   and repeat verification/review until no significant findings remain.
8. Update the deployment-identity knowledge-base topic and index only if this
   task yields reusable guidance, then commit that knowledge-base update.
9. Commit the implementation, push the task branch, open a pull request against
   the base branch, wait for required checks, and merge it.
