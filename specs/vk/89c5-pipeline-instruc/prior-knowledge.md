# Prior Knowledge: Task-Scoped Pipeline Instructions

**Task:** `vk/89c5-pipeline-instruc`

## Relevant project knowledge

1. `wiki/task-pipeline-block.md` establishes that selected pipeline stages are
   rendered verbatim into the task description as the generated `## Pipeline`
   block. There is no runtime layer that resolves placeholders or corrects a
   prompt after composition. The prompt itself must therefore explain how to
   derive `<task-id>`.
2. `docs/knowledge-base/prompt-driven-agent-pipelines.md` treats bundled prompt
   text as an executable contract. Collision-avoidance requirements should be
   explicit and protected by semantic assertions in pipeline loader tests.
3. `wiki/bundled-file-seed-manifests.md` and
   `docs/knowledge-base/prompt-driven-agent-pipelines.md` establish that bundled
   TOMLs are copied into user-editable storage and ordinarily are not
   overwritten once recorded. Updating an embedded asset changes fresh/reset
   defaults; automatically migrating existing customized files requires an
   explicit exact-byte migration. This task does not request such a migration,
   so existing reset behavior should remain the delivery mechanism.
4. `docs/knowledge-base/pipeline-settings-editor.md` confirms that bundled
   pipeline files are intentionally user-editable and resettable through the
   existing pipeline management surface. Prompt changes must not alter pipeline
   IDs, file IDs, or persistence semantics.
5. The current Vibe Kanban constitution uses Roman-numeral principles, while
   projects receiving generated pipeline blocks may use numbered principles.
   The constitution-stage instruction must therefore describe collision-safe
   behavior without assuming a particular current number or modifying the Vibe
   Kanban constitution solely to demonstrate the rule.

## Planning implications

- Change only `assets/pipelines/wikillm.toml`,
  `assets/pipelines/speckit.toml`, and focused Rust loader assertions.
- Keep stage order and all metadata stable.
- Make task-id derivation and base-branch-tip timing explicit in the prompt
  contracts.
- Do not change the Basic pipeline or any other service.

