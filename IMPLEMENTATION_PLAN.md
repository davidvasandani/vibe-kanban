# Implementation Plan: Refresh Active Workspace MCP Inventories

**Task:** `vk/d71c-refresh-active-w`

1. Establish the SpecKit constitution and task-scoped feature artifacts, using
   the technical spec and prior-knowledge distillation as inputs.
2. Trace the current refresh path from settings/connector mutation and UI action
   through server coordination, worker rematerialization, Codex app-server
   reload, next-turn confirmation, and model-visible tool registration.
3. Reproduce the stale-inventory failure with focused fixtures or a protocol
   harness that changes a stdio server's `tools/list` response while retaining
   one workspace session. Separately pin current HTTP/SSE behavior.
4. Identify the narrow broken boundary: effective-config selection, process
   ownership/routing, reload timing, stale control handoff, next-turn adoption,
   or status-source mismatch. Record protocol limitations rather than inferring
   unsupported generation/restart guarantees.
5. Implement the smallest end-to-end correction. Prefer a safe next-turn live
   reload when the executor demonstrably supports it; otherwise route the UI
   through the explicit restart fallback while preserving workspace and queued
   prompt state.
6. Make connector inventory and installation/enablement status derive from the
   effective assigned configuration, or make any intentional catalog-versus-
   installation distinction explicit and non-contradictory.
7. Add dependency-local tests for stdio tool addition, removal, and input-schema
   replacement, asserting the next model turn sees the new registry. Add failure
   retention/invalidating behavior, coordinator-worker routing coverage where
   applicable, UI state/restart coverage, and a non-stdio transport regression.
8. Regenerate checked-in types only through the repository generators if Rust
   API types change. Run dependency installation before formatting, then focused
   tests, generated-type checks, formatting, lint/check, and broader Rust tests
   proportionate to the diff.
9. Run SpecKit analysis, execute the dependency-ordered tasks via
   `/speckit.implement`, and tick each completed task with verification evidence.
10. Run an independent Codex diff review, address every confirmed significant
    finding, re-run affected checks, and repeat review until clean.
11. Update the Vibe Kanban knowledge base with reusable findings from the shipped
    fix (or state that none emerged), refresh its index, and commit the knowledge
    changes.
12. Rebase or otherwise verify against the latest base branch, push the task
    branch, open a pull request, monitor required checks, address failures, and
    merge the pull request. Do not change any non-Vibe-Kanban service.

No deployment change is planned unless investigation proves
`homelab/modules/vibe-kanban-rebuild.nix` must provide additional Vibe Kanban
runtime wiring.
