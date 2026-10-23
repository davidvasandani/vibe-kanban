# Implementation plan — vk/5de6-background-jobs

1. Confirm the reported background-job behavior and trace the affected executor
   from launch policy through delegation and parent completion. Use workspace
   `PRIOR_KNOWLEDGE.md` to distinguish pollers from agent subtasks.
2. Run the workspace SpecKit commands in their required order, using the
   per-task feature directory pointer. Resolve the scope of parent resumption
   during clarification before committing to a lifecycle design.
3. Implement the smallest supported tracked-child path and any required
   executor enforcement/instructions. Preserve foreground work and pollers.
4. Add regression coverage at the changed lifecycle boundary, including failure
   and repeated completion if a continuation mechanism changes.
5. Install locked dependencies, format as required by AGENTS.md, and run checks
   appropriate to the resulting changes.
6. Run independent Codex review; address significant findings and reverify.
7. Update and commit reusable project knowledge with this task identifier.
8. Open and merge the task PR against the confirmed base branch.

The user confirmed both behaviors. Concrete implementation and dependency
ordering are recorded in the workspace SpecKit feature directory
`homelab/specs/vk/5de6-background-jobs/`.
