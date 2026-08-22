# Implementation Plan: `list_all_messages`

1. Refresh the SpecKit constitution and create the task-scoped feature
   artifacts from `SPEC.md` and `PRIOR_KNOWLEDGE.md`.
2. Clarify the public meaning of “all”: all messages in the settled normalized
   projection for the selected execution, subject to existing per-entry text
   truncation and the documented legacy reconstruction bound.
3. Extend the shared messages HTTP query with an explicit all-messages flag and
   represent bounded-tail versus complete selection in the shared response
   builder without changing the recent-message defaults or cap.
4. Add focused server tests using more than 100 normalized entries to cover:
   bounded tail ordering/`has_more`, complete ordering/`has_more`, role
   filtering, and unchanged final-message semantics.
5. Refactor the MCP sessions tool implementation to share target resolution,
   workspace authorization, and HTTP fetching between `list_recent_messages`
   and the new `list_all_messages` tool.
6. Register `list_all_messages`, add it to the orchestrator exposure test, and
   document its usage and normalized-history boundary in the MCP crate guide.
7. Install locked dependencies if required, format the repository, and run
   focused Rust tests/checks followed by proportionate broader verification.
8. Run SpecKit analysis before implementation and execute the generated task
   list in dependency order, checking off each completed task.
9. Run an independent Codex diff review, address confirmed findings, and repeat
   verification/review until no significant findings remain.
10. Distill reusable architecture knowledge into the project knowledge base,
    update its index with task id `vk/29d8-vk-list-all-mess`, and commit it.
11. Commit the implementation, push the task branch, open a pull request against
    the base branch, wait for required checks, and merge it.
