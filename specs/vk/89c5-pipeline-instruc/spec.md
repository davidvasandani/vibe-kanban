# Feature Specification: Task-Scoped Pipeline Design Records

**Feature dir**: `specs/vk/89c5-pipeline-instruc/`
**Status**: Clarified

## Summary

Change the bundled WikiLLM + SpecKit workflow instructions so concurrent tasks
write their technical specification, recalled knowledge, and implementation plan
inside their own task directory, and so constitution additions choose principle
numbers from the latest base-branch state at merge time. This prevents routine
cross-task file and numbering conflicts while keeping each task's design record
together.

## User Stories

- As a developer running concurrent tasks in one repository, I want each task's
  design artifacts isolated by task ID so that merging one task does not
  conflict with another task's design files.
- As a maintainer reviewing a task, I want its WikiLLM and SpecKit artifacts in
  one directory so that the design record is easy to discover and audit.
- As a developer adding a constitution principle, I want the pipeline to make me
  re-check the latest base branch before choosing its number so that unmerged
  branches do not publish duplicate principle identifiers.

## Functional Requirements

- FR-1: The WikiLLM technical-spec stage must direct the agent to
  `specs/vk/<task-id>/technical-spec.md`.
- FR-2: The WikiLLM recalled-knowledge stage must direct the agent to
  `specs/vk/<task-id>/prior-knowledge.md`.
- FR-3: The WikiLLM implementation-plan stage must direct the agent to
  `specs/vk/<task-id>/implementation-plan.md`.
- FR-4: Each affected WikiLLM prompt must define `<task-id>` as the current
  task's identifier rather than leaving it as an unexplained literal.
- FR-5: The SpecKit constitution stage must mark a principle number assigned at
  draft time as provisional and direct the agent to inspect the latest
  base-branch tip immediately before merge.
- FR-6: The WikiLLM and SpecKit merge stages must direct the agent to renumber
  its own unmerged principle and update internal references when the provisional
  number is already present on the latest base tip.
- FR-7: Pipeline stage order, identifiers, labels, enabled defaults, and prompt
  fragments unrelated to artifact paths or constitution-number collision safety
  must remain unchanged.
- FR-8: Automated checks must verify the new path and numbering contracts in the
  loaded bundled pipelines.

## Out of Scope

- Changing another service or the shared homelab deployment.
- Moving historical task artifacts.
- Automatically rewriting user-customized pipeline TOMLs already copied to
  machine-local storage.
- Changing the Basic pipeline.
- Renumbering principles that have already merged.

## Acceptance Criteria

- [x] Loading WikiLLM returns all three task-scoped artifact paths with explicit
      current-task ID guidance.
- [x] Loading SpecKit returns the base-tip check and unmerged-renumbering rule in
      its constitution stage.
- [x] Focused pipeline service tests pass.
- [x] No unrelated pipeline metadata or other service is changed.

## Open Questions

None.
