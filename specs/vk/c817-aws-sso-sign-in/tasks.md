# Tasks
Command: /speckit.tasks
Plan: ./plan.md

- [x] T001 Record spec, knowledge recall, constitution review, clarification and plan.
- [x] T002 [P] Implement shared AWS migration helper and regression tests in `homelab/modules/vibe-kanban-aws-state.py` and `homelab/tests/vibe-kanban-aws-state.py`.
- [x] T003 [P] Add live AWS overlay regression in `crates/worker/src/execution.rs`.
- [x] T004 [P] Correct host-scope labels in `packages/web-core/src/i18n/locales/*/settings.json`.
- [x] T005 Wire migration service and AWS CLI PATH in `homelab/modules/vibe-kanban-rebuild.nix`; extend `homelab/tests/vibe-kanban-cluster.nix` (depends T002).
- [ ] T006 Run Python/Nix/Rust/frontend checks and required format; record live acceptance evidence or limits (depends T002–T005).
- [x] T007 Independent Codex CLI review; fix significant findings and re-verify (depends T002–T005; may overlap pending verification, all checks required before merge).
- [x] T008 Update `wiki/managed-cli-tool-catalog.md`, add AWS topic and refresh `wiki/INDEX.md`, tag and commit knowledge (depends T007).
- [ ] T009 Open and merge scoped repository PRs against base branches (depends T008).
