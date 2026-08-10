# Technical Specification: Task-Scoped WikiLLM Artifacts

**Task:** `vk/89c5-pipeline-instruc`

## Problem

The bundled WikiLLM pipeline instructs every concurrent task to create
`SPEC.md` and `IMPLEMENTATION_PLAN.md` at the repository root. Tasks sharing a
repository therefore modify the same tracked paths, causing avoidable merge
conflicts and forcing design records to be relocated manually. Its recall stage
similarly targets `PRIOR_KNOWLEDGE.md` at the workspace root, which can collide
even when the file is not committed. Constitution updates can also choose the
same principle number when parallel branches inspect stale branch tips.

## Scope

Update only the Vibe Kanban service's bundled WikiLLM and SpecKit pipeline
instructions and their tests. No other service or deployment configuration is
required.

## Requirements

1. WikiLLM's technical-spec stage must write to
   `specs/vk/<task-id>/technical-spec.md`.
2. WikiLLM's prior-knowledge stage must write to
   `specs/vk/<task-id>/prior-knowledge.md` so all task design artifacts are
   isolated together.
3. WikiLLM's implementation-plan stage must write to
   `specs/vk/<task-id>/implementation-plan.md`.
4. Prompts must make clear that `<task-id>` is the task identifier from the
   current task/branch, preventing agents from treating it as a literal folder.
5. SpecKit's constitution stage must mark draft-time principle numbers as
   provisional, and WikiLLM/SpecKit merge stages must instruct the agent to
   inspect the latest base-branch tip immediately before merge, renumber an
   unmerged collision, and update its internal references.
6. Automated tests must lock in these collision-avoidance instructions.
7. Existing pipeline stage ordering, identifiers, labels, defaults, and prompts
   unrelated to artifact paths or constitution-number safety must remain
   unchanged.

## Acceptance Criteria

- Loading the bundled WikiLLM pipeline yields task-scoped paths for `spec`,
  `recall-knowledge`, and `plan`.
- Loading the bundled SpecKit pipeline yields the merge-time principle-number
  collision guard in the `constitution` prompt.
- Focused pipeline service tests pass.
- The change is limited to Vibe Kanban pipeline assets, tests, and task design
  or knowledge records.

## Risks and Mitigations

- **Placeholder ambiguity:** explicitly define `<task-id>` in each affected
  prompt.
- **Only new installs receive bundled text:** preserve existing bundle seeding
  behavior; users can use the existing reset action to restore bundled prompts.
- **Over-broad product change:** do not alter Basic pipeline behavior because
  this request targets the WikiLLM + SpecKit flow.
