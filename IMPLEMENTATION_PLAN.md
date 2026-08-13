# Implementation Plan: Commits Behind in the Git Header

1. Establish the task's SpecKit constitution and feature artifacts under a new
   `specs/vk/a35b-commits-behind-m/` directory, retaining the request's scope and
   the prior-knowledge constraints.
2. Inspect the branch-status hook/query lifecycle, `RightSidebar` header-extra
   composition, and existing UI test conventions to choose a presentation seam
   that remains live while the Git body is collapsed.
3. Add a small, testable Git-header status derivation/presentation component
   that:
   - joins repository metadata to branch status by repository ID;
   - omits zero, missing, and loading values;
   - shows only the count for a single-repository workspace;
   - shows repository names with counts for a multi-repository workspace; and
   - bounds/truncates visible text while preserving full accessible context.
4. Mount that component through the existing Git section `headerExtra` in
   `RightSidebar` without changing the drawer's expansion or flex behavior.
5. Add focused tests for empty/zero state, one repository, multiple
   repositories, ID-based matching, and collapsed-header availability.
6. Run the targeted frontend tests, formatting, type checks, and linting; resolve
   any regressions within Vibe Kanban scope.
7. Run the required independent Codex review, address confirmed significant
   findings, and repeat verification until no significant findings remain.
8. Distill genuinely reusable UI/query-boundary knowledge into the Vibe Kanban
   knowledge base, tag it with `vk/a35b-commits-behind-m`, refresh the index,
   and commit the knowledge-base update.
9. Commit the complete task, open a pull request against the latest base branch,
   verify its checks and review state, merge it, and report the merged result.
