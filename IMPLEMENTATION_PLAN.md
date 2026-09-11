# Implementation Plan: Claude Fable 5.1 and Claude Code Refresh

**Task:** `vk/1f95-update-claude-mo`

1. Inspect the current Claude executor catalog, package pin, version-coupled
   tests/comments, governing Nix module, and clean/dirty state of both scoped
   repositories.
2. Verify the Fable 5.1 model identifier, supported effort controls, current
   Claude Code release, release notes, and alias mappings using authoritative
   Anthropic/npm/native-package evidence.
3. Run the SpecKit constitution, specification, clarification, planning, task,
   and analysis stages, saving their task-scoped artifacts under
   `specs/vk/1f95-update-claude-mo/`.
4. Update the Claude executor's fallback model catalog and focused tests for
   Fable 5.1, preserving all still-supported existing choices and the default.
5. Update the pinned `@anthropic-ai/claude-code` version and every deliberate
   version twin in comments/tests.
6. Inspect the refreshed native Claude Code binary for the exact scheduling and
   background tool names and aliases. Adjust Vibe Kanban's safety constants
   only if verified runtime facts changed, while retaining the parameter-level
   background-Bash guard.
7. Update `homelab/modules/vibe-kanban-rebuild.nix` only if the refreshed Vibe
   Kanban executor requires a deployment/runtime change; otherwise record that
   the module was inspected and remains compatible.
8. Install dependencies with `pnpm install --frozen-lockfile`, format, and run
   focused Claude executor tests plus generated-type and relevant broader
   checks. If homelab changes, run its narrow Nix parse/format/evaluation gates.
9. Run an independent Codex CLI review of the complete task diff. Resolve each
   confirmed significant finding and repeat review/verification until clean.
10. Update the Vibe Kanban project knowledge base with reusable model/CLI bump
    guidance, tag it `vk/1f95-update-claude-mo`, refresh the index, and commit
    the knowledge-base change.
11. Reconcile and complete all pipeline artifacts and task checkboxes, confirm
    scope and repository status, commit the implementation, push the branch,
    open a pull request against the latest base branch, wait for required
    checks, and merge it.
