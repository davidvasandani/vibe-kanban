# Implementation Plan: Upgrade Codex Models

1. Establish the project constitution, then generate and clarify the SpecKit
   feature specification from the technical spec and recalled knowledge.
2. Produce the SpecKit technical plan, supporting research/contract artifacts,
   and a dependency-ordered task list; analyze them for gaps and constitution
   violations before editing product code.
3. In `crates/executors/src/executors/codex.rs`, replace the advertised model
   vector with the six visible models reported by Codex 0.155.1 `model/list`.
4. Reuse the existing `max` and `ultra` reasoning-option builders so each
   model's picker options exactly match app-server capabilities.
5. Update the exact ordered catalog regression test for IDs, labels, order,
   and effort sets. Leave transport/session fixtures that use older model
   strings unchanged because they test pass-through behavior.
6. Run Rust formatting, the focused Codex catalog test, generated-type checks,
   and relevant executor/package checks; inspect the diff for unrelated files.
7. Run an independent Codex diff review. Address confirmed findings and repeat
   verification and review until no significant findings remain.
8. Update `wiki/executor-model-catalogs.md` with any reusable compatibility
   lesson from the shipped refresh, add this task ID, and keep `wiki/INDEX.md`
   synchronized if its summary changes.
9. Commit the Vibe Kanban implementation and knowledge artifacts, push the task
   branch, open a pull request to the base branch, wait for required checks,
   resolve any failures, and merge the pull request.

## Dependency Layers

- Layer 1: SpecKit stages 4-9, sequential by pipeline contract.
- Layer 2: Product code and focused test update.
- Layer 3: Formatting, tests, generated checks, and diff inspection.
- Layer 4: Independent review and any review-driven corrections.
- Layer 5: Knowledge capture, commit, pull request, CI, and merge.

No implementation tasks are safely parallel within this small one-file change;
the test assertion and catalog implementation are one contract and should land
together.
