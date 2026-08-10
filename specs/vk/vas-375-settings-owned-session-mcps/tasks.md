# Tasks: Settings-Owned MCPs in Every New Session

## Layer 1 — Producer and Isolation Foundations

- [x] T001 Generalize coordinator dispatch snapshot creation to all MCP-capable
  selected executors in `crates/local-deployment/src/container.rs`.
- [x] T002 [P] Refactor worker prepared MCP state to retain execution root,
  native target path, adapter, and child launch environment.
- [x] T003 [P] Add a safe execution-scoped home overlay builder that excludes
  the native target config while preserving unrelated home assets.

## Layer 2 — Consumer Integration

- [x] T004 Materialize snapshots for every supported executor through the native
  adapter and apply child-only home/config environment overrides.
- [x] T005 Retarget confirmed Codex refresh to the prepared native config while
  preserving its executor-specific lifecycle checks.
- [x] T006 Ensure drop cleanup removes the complete execution-scoped overlay.

## Layer 3 — Regression Coverage and Deployment Cleanup

- [x] T007 [P] Add coordinator tests proving non-Codex profiles receive the
  correct executor-bound snapshot.
- [x] T008 [P] Add worker tests for Claude/Codex/Gemini paths, unrelated asset
  preservation, concurrent isolation, mismatch rejection, and cleanup.
- [x] T009 [P] Remove only the competing Vibe Kanban entry from
  `homelab/.mcp.json` and validate the JSON.

## Layer 4 — Verification and Delivery

- [x] T010 Run formatting and focused Rust tests/checks; fix confirmed failures.
- [x] T011 Run independent Codex review, address significant findings, and
  re-verify until clean.
- [x] T012 Record reusable project knowledge, tag it with VAS-375, refresh the
  knowledge index, and commit the knowledge-base update.
- [ ] T013 Publish coordinated Vibe Kanban and homelab PRs, merge them, and report
  credential-rotation and deployment verification requirements.
