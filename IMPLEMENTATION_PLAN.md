# Implementation Plan: Remote-to-Local Workspace Archive Reconciliation

1. Inspect provider composition, workspace shape types, existing hook test
   conventions, and local API transport behavior to choose a shared seam that
   has both remote workspace rows and local workspace summaries.
2. Run the SpecKit constitution, specification, clarification, planning, task,
   and analysis commands; incorporate any constraints or gaps they identify.
3. Add a pure selector that returns unique local workspace IDs whose linked
   remote records are archived while their local records are still active.
4. Add a reconciliation hook that calls the existing local workspace update API
   for selected IDs, tracks requests in flight to prevent duplicate archive
   submissions, isolates per-workspace failures, and permits later retries.
5. Wire the hook into the remote project data provider so Electric updates,
   fallback snapshots, reconnects, and provider remounts all trigger the same
   level-based reconciliation.
6. Add focused unit tests for mismatch selection and reconciliation behavior,
   including ignored remote-only/unlinked/already-archived cases, duplicate
   links, in-flight deduplication, independent failures, and retryability.
7. Run focused tests, frontend type checking, and repository formatting; inspect
   the final diff for generated or unrelated changes.
8. Run an independent Codex diff review, fix every confirmed significant
   finding, and repeat verification/review until no significant findings remain.
9. Update the existing issue-status-side-effects knowledge page with the shipped
   cross-boundary reconciliation pattern, tag it with `f464-vk-workspace-mgm`,
   refresh the knowledge index, and commit the knowledge-base update before
   marking the task ready.
