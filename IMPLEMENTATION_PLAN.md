# Implementation plan

Task: vk/113f-sidebar-randomly

1. Complete the ordered SpecKit constitution, specification, clarification, plan, tasks, and analysis stages using workspace command instructions.
2. Fix `packages/web-core/src/shared/hooks/useWorkspaces.ts`: reject failed summary requests so React Query retains successful cached metadata; accept authoritative empty results; remove cross-host placeholder reuse.
3. Add hook-level regression tests using a real QueryClient for active/archived failures, automatic refresh recovery, explicit clearing, initial failure, and host isolation.
4. Install frozen dependencies, run targeted tests, frontend checks/lint, required format, and broader checks as feasible; document actual results.
5. Run independent Codex CLI review, resolve significant findings and verify again.
6. Record reusable query-cache authority knowledge and task tag in the project knowledge base and index; commit.
7. Open and merge a PR against the repository base branch after required checks pass.

No infrastructure changes are planned. SpecKit workspace command outputs belong at `homelab/specs/vk/113f-sidebar-randomly/`; service implementation and root spec/plan belong in `vibe-kanban/`.
