# Implementation Plan: Scrollable Create-Issue Settings

**Task ID:** `vk/4f69-vk-create-issue`

1. Refresh the repository SpecKit constitution and create the task's feature
   artifacts under a task-specific `specs/vk/` directory.
2. Reconcile the feature specification with `SPEC.md`, the supplied mobile
   screenshot, `PRIOR_KNOWLEDGE.md`, and the existing issue-panel host/layout
   hierarchy; resolve ambiguities without expanding beyond Vibe Kanban.
3. Produce the SpecKit technical plan, research, contracts/data-model notes,
   dependency-ordered tasks, and pre-implementation analysis.
4. Add focused regression coverage for the issue-panel layout contract:
   fixed/clipped shell, shrinkable vertically scrolling content, and create
   controls contained within that scroll region.
5. Update the shared `KanbanIssuePanel` flex sizing so constrained mobile and
   desktop hosts allow the content child to shrink and scroll while the header
   remains fixed.
6. Install frozen workspace dependencies if required, then run the focused UI
   test, formatting, relevant frontend type/lint verification, and
   `git diff --check`.
7. Run an independent Codex diff review, address confirmed significant
   findings, and repeat verification/review until clean.
8. Record the reusable nested-flex scroll-containment pattern and regression
   approach in `docs/knowledge-base/`, tag it `vk/4f69-vk-create-issue`, refresh
   the index, and commit that knowledge-base update.
9. Merge the completed task branch into its base branch after confirming both
   repositories remain scoped correctly and clean apart from intended work.

Steps 4 and 5 will be represented in the dependency-ordered SpecKit task list;
tests should land before or alongside the implementation so the regression is
demonstrable. No deployment/IaC update is expected because this is a shared
frontend source fix only.
