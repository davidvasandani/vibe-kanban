# Tasks: Align Codex Model Catalog with the Current Selector

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes.

## Phase 1: Contract (test first)
- [x] T001 Update the expected vector in `discovered_models_match_current_chatgpt_catalog` to the ten-entry contract (`contracts/codex-model-catalog.md`) in `crates/executors/src/executors/codex.rs`. Run it and confirm it fails against the current catalog.

## Phase 2: Core
- [x] T002 Prepend `gpt-6.2-astra`, `gpt-6.2-sol`, `gpt-6.1-sol`, `gpt-6.1-terra` (ultra efforts) to the models vector in `Codex::discover_options`, `crates/executors/src/executors/codex.rs` (depends on T001)

## Phase 3: Validation & knowledge
- [x] T003 Run `cargo test -p executors discovered_models_match_current_chatgpt_catalog`, then `cargo test -p executors`, `pnpm run generate-types:check` and `pnpm run format`. Record the results in `specs/vk/b1ec-align-codex-mode/verification.md` (depends on T002)
- [x] T004 [P] Update the compatibility rule (selector target, client-version gate, order divergence) and the contributor list in `wiki/executor-model-catalogs.md` (depends on T002)
- [x] T005 [P] Refresh the `executor-model-catalogs` entry in `wiki/INDEX.md` (depends on T002)

## Phase 4: Review & ship gate
- [x] T006 Run the independent Codex review of the diff and iterate. Record it in `specs/vk/b1ec-align-codex-mode/review.md` (depends on T003–T005)
- [x] T007 Open the PR. Merge only after an explicit operator decision on the deployed-CLI gate (spec open question) (depends on T006)
