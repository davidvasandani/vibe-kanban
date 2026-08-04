# Tasks: Vibe Kanban Soft Restarts

**Plan**: `./plan.md`

The analysis stage narrowed implementation to the existing production architecture: the cluster worker is already the stable process owner, so this feature prevents deployments from replacing it while it owns work and makes coordinator reconnects non-destructive in the browser.

## Phase 1: Baseline and ownership contract

- [x] T001 Install locked dependencies and inspect coordinator/worker execution, replay, reconciliation, terminal, and deployment ownership paths.
- [x] T002 Distill existing knowledge and record the stable-owner design in `PRIOR_KNOWLEDGE.md`, `research.md`, `data-model.md`, and `contracts/`.
- [x] T003 Refresh `.specify/memory/constitution.md` with the stable process-owner and explicit soft/hard lifetime invariant.

## Phase 2: Worker drain protocol

- [x] T004 Expose authoritative active execution counts and verify count transitions in `crates/worker/src/execution.rs`.
- [x] T005 Confirm PTY reconnection remains explicitly out of scope.
- [x] T006 Add persistent SIGUSR1/SIGUSR2 admission drain control and SIGTERM handling in `crates/worker/src/main.rs`.
- [x] T007 Reject new dispatch with retryable service-unavailable responses while preserving idempotent retries in `crates/worker/src/execution.rs` and `crates/worker/src/worker_api.rs`.
- [x] T008 Publish `active_execution_count`, `admission_draining`, and `drain_safe` on worker `/health` in `crates/worker/src/lib.rs`.
- [x] T009 Add regression coverage for drain safety, idempotent retry, and a coordinator polling gap in `crates/worker/src/execution.rs` and `crates/worker/src/lib.rs`.

## Phase 3: Safe deployment activation

- [x] T010 Make worker distribution close admission, wait for acknowledgement, and defer release activation while work is active in `homelab/modules/vibe-kanban-rebuild.nix`.
- [x] T011 Persist drain across the worker restart, resume admission only after health succeeds, retain rollback, and fail closed for pre-protocol workers in `crates/worker/src/main.rs` and `homelab/modules/vibe-kanban-rebuild.nix`.
- [x] T012 Parse-check `homelab/modules/vibe-kanban-rebuild.nix` and preserve the legacy coordinator release/static health gate.

## Phase 4: Browser recovery

- [x] T013 Preserve initialized JSON-patch snapshots across same-endpoint reconnect attempts and add bounded jittered backoff in `packages/web-core/src/shared/hooks/useJsonPatchWsStream.ts`.
- [x] T014 Surface workspace-stream connection state through `WorkspaceProvider` and render an accessible, additive reconnect banner in `SharedAppLayout.tsx`.
- [x] T015 Add regression tests for retained content, reconnect state, backoff bounds, and banner visibility under `packages/web-core/src/shared/hooks/` and `packages/web-core/src/shared/components/ui-new/containers/`.

## Phase 5: Verification, review, and knowledge

- [x] T016 Run formatting, worker tests/checks, web-core tests/typecheck/lint, diff checks, and Nix parse/evaluation; record exact results in `verification.md`.
- [x] T017 Run independent Codex CLI review, fix confirmed significant findings, and repeat until clean; record results in `review.md`.
- [ ] T018 Update and commit the project knowledge base, tagged `vk/9632-vk-soft-restarts`, and refresh `docs/knowledge-base/INDEX.md`.
