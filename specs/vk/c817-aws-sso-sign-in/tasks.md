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

- [x] T014 Lazily admit profile probes and share executable discovery per refresh.
- [x] T015 Verify long overlapping batches, format, independently review, and record evidence.
- [x] T016 Update knowledge, merge the fix, and recheck deployed statuses.

- [x] T017 After fresh Settings sign-in, verify deployed profiles and agent CLI/SDK authentication. At 17:25 UTC all 31 profiles correctly reported unauthenticated, with zero unknown results; agent CLI confirmed session expiry.

Final acceptance passed after fresh Settings sign-in: deployed revision `679b284` returned all 31 profiles authenticated; sanitized agent CLI STS and Node SDK default-provider checks both succeeded.

## MCP follow-up — vk/669e-mcp-blocks-progr

Applied `/speckit.tasks`. These tasks follow the MCP plan addendum; earlier task checkboxes are historical.

- [x] T901 Correlate execution and source evidence in `mcp-research.md`.
- [x] T902 Set metadata-only fork/resume requests and add wire regressions in `crates/executors/src/executors/codex.rs` (depends on T901).
- [x] T903 Verify protocol behavior, run focused tests and formatting; document results in `mcp-research.md` (depends on T902).
- [x] T904 Run independent Codex review and address findings; document results in `mcp-research.md` (depends on T903).
- [x] T905 Update `docs/knowledge-base/cluster-mcp-runtime-connectivity.md` and `docs/knowledge-base/INDEX.md`, commit knowledge and implementation, and open PR (depends on T904). Final merge follows successful CI in pipeline stage 13.

No [P] implementation tasks: the change and its regressions share one file. Stable T9xx identifiers distinguish this follow-up from the existing AWS tasks.
