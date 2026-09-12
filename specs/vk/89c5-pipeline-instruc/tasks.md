# Tasks: Task-Scoped Pipeline Design Records

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group.

## Phase 1: Prompt contracts

- [x] T001 [P] Update the three artifact-producing WikiLLM prompt fragments in
  `assets/pipelines/wikillm.toml` with task-scoped paths and explicit task-ID
  derivation.
- [x] T002 [P] Update the SpecKit constitution and merge prompts in
  `assets/pipelines/speckit.toml` plus the WikiLLM merge prompt in
  `assets/pipelines/wikillm.toml` with the provisional-number and latest-base-tip
  collision rule.

## Phase 2: Regression coverage

- [x] T003 Add focused bundled WikiLLM and SpecKit semantic assertions in
  `crates/services/src/services/pipelines/mod.rs` after T001 and T002, preserving
  the Basic pipeline's existing verbatim assertion.

## Phase 3: Verification

- [x] T004 Run Rust formatting and the focused pipeline service tests after
  T003; record evidence in `specs/vk/89c5-pipeline-instruc/verification.md`.
- [x] T005 Reconcile the final diff against `SPEC.md`,
  `IMPLEMENTATION_PLAN.md`, `specs/vk/89c5-pipeline-instruc/spec.md`,
  `plan.md`, `contracts/prompt-contract.md`, and the constitution after T004.

## Phase 4: Independent review and knowledge

- [x] T006 Run the independent Codex diff review, fix confirmed findings, and
  repeat verification until no significant findings remain; record the result
  in `specs/vk/89c5-pipeline-instruc/review.md` after T005.
- [x] T007 Update the relevant project knowledge page and index with reusable
  task-scoped prompt guidance, tag it with `89c5-pipeline-instruc`, and commit
  the knowledge base after T006.

## Phase 5: Integration

- [x] T008 Merge branch `vk/89c5-pipeline-instruc` into its base branch after
  T007.
