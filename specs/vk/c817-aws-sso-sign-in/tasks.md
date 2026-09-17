# Tasks
Command: /speckit.tasks
Plan: ./plan.md

- [x] T001 Record spec, knowledge recall, constitution review, clarification and plan.
- [x] T002 [P] Implement shared AWS migration helper and regression tests in `homelab/modules/vibe-kanban-aws-state.py` and `homelab/tests/vibe-kanban-aws-state.py`.
- [x] T003 [P] Add live AWS overlay regression in `crates/worker/src/execution.rs`.
- [x] T004 [P] Correct host-scope labels in `packages/web-core/src/i18n/locales/*/settings.json`.
- [x] T005 Wire migration service and AWS CLI PATH in `homelab/modules/vibe-kanban-rebuild.nix`; extend `homelab/tests/vibe-kanban-cluster.nix` (depends T002).
- [x] T006 Run Python/Nix/Rust/frontend checks and required format; record live acceptance evidence or limits (depends T002–T005).
- [x] T007 Independent Codex CLI review; fix significant findings and re-verify (depends T002–T005; may overlap pending verification, all checks required before merge).
- [x] T008 Update `wiki/managed-cli-tool-catalog.md`, add AWS topic and refresh `wiki/INDEX.md`, tag and commit knowledge (depends T007).
- [x] T009 Open and merge scoped repository PRs against base branches (depends T008).

Delivery: deployment PR #1240 and application PR #302 merged. Login-shell follow-up: homelab PR #1242. After a fresh Settings sign-in, live agent CLI STS and Node default-provider authentication both passed without pasted credentials.

## Status-probe follow-up
- [x] T010 Specify the fix from live probe timing evidence in `SPEC.md`, `IMPLEMENTATION_PLAN.md`, and `spec.md`.
- [x] T011 Implement shared concurrency and separate queue/execution budgets with regressions in `crates/services/src/services/aws_sso.rs`.
- [x] T012 Run verification and independent Codex review; record evidence in `validation.md` and `review.md`.
- [x] T013 Update `wiki/aws-sso-agent-state.md`, commit knowledge, open and merge the follow-up PR, and check deployed statuses.

Deployment check: revision `d154bab` is running on the coordinator. Live all-profile acceptance is **not passing** under observed CPU contention: isolated refresh returned 4 authenticated and 27 unknown. See validation.md; do not treat merge/deployment as proof of all-profile authentication.
