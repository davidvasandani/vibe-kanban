# Tasks: Claude Fable 5.1 and Current Claude Code

**Plan:** `./plan.md`

Tasks are dependency ordered. `[P]` marks work that can be completed within the
same dependency layer without overlapping files.

## Phase 1: Evidence and implementation

- [x] **T001** Verify Fable 5.1's exact model ID and effort capabilities from
  Anthropic documentation and the current native Claude Code artifact; record
  the decisions in `clarifications.md` and `research.md`.
- [x] **T002** Verify npm channel state, select the latest non-prerelease Claude
  Code pin, review its changelog, and record the decision in `research.md`.
- [x] **T003** Verify the scheduling/background canonical names and aliases
  against the selected native artifact; record the results in
  `clarifications.md` and `research.md`.
- [x] **T004** Update the Fable 5.1 catalog entry, Claude Code pin, and all
  deliberate version twins in `crates/executors/src/executors/claude.rs`
  (depends on T001-T003).
- [x] **T005** Add focused catalog/effort/version/safety regression coverage in
  `crates/executors/src/executors/claude.rs` (depends on T004).
- [x] **T006** Inspect `homelab/modules/vibe-kanban-rebuild.nix` for Node 22 and
  command-override compatibility; make a Vibe-Kanban-only deployment change if
  required (depends on T002).

## Phase 2: Verification

- [x] **T007** Install the locked pnpm dependency graph, then run focused Claude
  executor tests (depends on T004-T006).
- [x] **T008 [P]** Run generated-type checks and inspect any generated diff;
  retain only source-driven changes (depends on T004-T006).
- [x] **T009 [P]** Run repository formatting, `git diff --check`, and relevant
  broader Rust/project checks (depends on T004-T006).
- [x] **T010 [P]** If T006 changes homelab, run narrow Nix parse, format, and
  Vibe Kanban configuration evaluation checks (depends on T006).

## Phase 3: Analysis, review, and delivery

- [x] **T011** Reconcile spec, plan, tasks, constitution, and implementation;
  record the cross-check in `analyze.md` (depends on T007-T010).
- [x] **T012** Run an independent Codex CLI review of the complete diff, fix
  every confirmed significant finding, and repeat verification/review until
  clean; record the result in `review.md` (depends on T011).
- [x] **T013** Update a reusable Vibe Kanban knowledge-base page tagged
  `vk/1f95-update-claude-mo`, refresh `docs/knowledge-base/INDEX.md`, and commit
  the knowledge base (depends on T012).
- [x] **T014** Complete acceptance/task status and final verification evidence,
  commit remaining scoped changes, and confirm the actual base tip and
  constitution before delivery (depends on T013).
- [ ] **T015** Push the task branch, open a pull request against the base branch,
  wait for required checks, and merge it (depends on T014).
