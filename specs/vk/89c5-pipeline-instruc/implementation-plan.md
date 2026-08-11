# Implementation Plan: Task-Scoped WikiLLM Artifacts

**Task:** `vk/89c5-pipeline-instruc`

1. Refresh the Vibe Kanban SpecKit constitution and record any task-specific
   principle only if the existing constitution lacks a governing invariant.
2. Create the task's SpecKit feature specification under
   `specs/vk/89c5-pipeline-instruc/`, then resolve all material prompt wording
   questions.
3. Produce the SpecKit technical plan, research notes, contracts where useful,
   and a dependency-ordered task list in that same directory.
4. Cross-check the root technical spec, prior-knowledge distillation, SpecKit
   artifacts, current constitution, and requested scope before editing code.
5. Update WikiLLM's bundled `spec`, `recall-knowledge`, and `plan` prompt
   fragments to use the task-scoped design-record directory and explain the
   `<task-id>` placeholder.
6. Update SpecKit's bundled `constitution` prompt to mark draft-time principle
   numbers as provisional, and update WikiLLM/SpecKit merge prompts to recheck
   the latest base tip, renumber an unmerged collision, and repair references.
7. Replace or extend focused pipeline-loader tests so the semantic contract is
   pinned without changing unrelated Basic-pipeline compatibility coverage.
8. Run formatting and focused Rust tests; inspect the final diff for scope.
9. Run the independent Codex review loop and address confirmed findings.
10. Distill the reusable prompt-contract rule into the Vibe Kanban knowledge
    base, refresh its index, and commit the knowledge record as required.
11. Merge the task branch into its base branch after implementation and review
    are complete.
