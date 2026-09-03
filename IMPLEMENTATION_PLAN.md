# Implementation Plan: Recover Missing Codex Conversations

**Task:** `vk/af0d-no-conversation`

1. Refresh the Vibe Kanban constitution and write the task-scoped SpecKit
   specification artifacts.
2. Confirm the pinned Codex app-server's exact JSON-RPC response for a missing
   thread and inspect how `JsonRpcPeer` currently erases error structure.
3. Clarify the recovery boundary: normal chat follow-ups recover; unrelated
   errors and semantically different operations remain fail-loud.
4. Preserve enough JSON-RPC error detail in `ExecutorError` or a narrowly scoped
   client result to classify the upstream not-found case without broad string
   matching.
5. Extract/test the resume-or-start decision so a successful fork is unchanged,
   an exact missing-conversation response invokes `thread/start` with the same
   parameters, and all other failures propagate.
6. Keep the common post-resolution path unchanged: set resolved model, register
   the replacement thread (which emits/persists its ID), select collaboration
   mode, and submit the current prompt.
7. Add focused Rust regression tests for classifier false positives and the
   fallback request sequence. Run formatting and targeted executor tests, then
   broader checks proportionate to the changed boundary.
8. Run SpecKit analysis, execute and tick the dependency-ordered tasks, and
   record verification evidence.
9. Run an independent Codex diff review, address confirmed findings, and repeat
   until no significant findings remain.
10. Update the Vibe Kanban knowledge base with the reusable recovery/error-
    classification rule (or explicitly record that none emerged), refresh its
    index, and commit it.
11. Push the task branch, open a pull request against the current base branch,
    verify the latest base tip immediately before merge, and merge the pull
    request.

No change to `homelab/modules/vibe-kanban-rebuild.nix` is planned because the
fix is expected to be entirely within the Vibe Kanban executor.
