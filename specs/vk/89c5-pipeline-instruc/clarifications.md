# Clarifications: Task-Scoped Pipeline Design Records

1. **Does task scoping include `PRIOR_KNOWLEDGE.md`?** Yes. The issue identifies
   the same collision class for stage 2 and asks that it be considered. Keeping
   it at the workspace root would leave the workflow only partially safe under
   concurrency and split the design record. The canonical name is
   `specs/vk/<task-id>/prior-knowledge.md`.

2. **Should existing installed pipeline files be rewritten automatically?** No.
   Bundled pipeline TOMLs are user-editable. Existing project knowledge requires
   exact historical-byte migration data before an automatic overwrite can be
   safe, and this issue requests prompt corrections rather than a persistence
   migration. Fresh installs receive the new defaults; existing users can apply
   them through the established reset action.

3. **What does “latest main” mean for non-`main` repositories?** Use the task's
   actual base branch. The prompt will say “latest base-branch tip” and identify
   `main` as the common example, preserving the issue's intent without assuming
   every repository uses the same branch name.

4. **When is a constitution renumbering required?** Immediately before assigning
   or finalizing a new principle number, fetch/inspect the latest base tip. If
   the candidate number is already present there, renumber the unmerged addition
   to the next free number. Never renumber an already-merged principle as part of
   collision resolution.

No open questions remain.
