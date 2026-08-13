# Implementation Plan: Stable Workspace Order During Restart

1. Establish the task's SpecKit constitution and feature artifacts under a new
   `specs/vk/9391-workspace-order/` directory, incorporating the technical spec
   and prior-knowledge findings.
2. Trace the workspace stream, summary-query, sidebar transformation, and sort
   lifecycle to confirm which persisted timestamp is available before summaries
   load and document any ambiguity during clarification.
3. Extract or introduce a focused workspace comparator that preserves pinning,
   selected sort direction, and deterministic ties while using `updatedAt` as a
   fallback for missing latest-process completion data.
4. Change missing/invalid selected timestamps to sort after known timestamps,
   applying the same rule to active and archived lists.
5. Add focused unit tests for startup fallback ordering, summary precedence,
   ascending and descending sorts, pinned workspaces, invalid timestamps, and
   stable tie-breaking.
6. Install dependencies if needed, format the repository, and run targeted
   frontend tests plus proportionate type/lint checks.
7. Run an independent Codex CLI review of the diff, address every confirmed
   significant finding, and repeat review and verification until clean.
8. Add reusable partial-data workspace ordering guidance to the project
   knowledge base, tag it with `vk/9391-workspace-order`, refresh the index, and
   commit the knowledge-base update.
9. Commit the task, open a pull request against the base branch, monitor required
   checks, merge the pull request, and report the merged result.
