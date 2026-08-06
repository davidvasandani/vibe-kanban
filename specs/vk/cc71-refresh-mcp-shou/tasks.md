# Tasks: Refresh Active Remote MCP Snapshots

**Plan**: `./plan.md`

Tasks are dependency-ordered by layer. Tasks marked **[P]** touch independent
files and may run together within that layer.

## Layer 1: Baseline and contracts

- [ ] T001 Bring the task branch forward to the VAS-356-capable `origin/main`
      baseline, preserving task documents and checking the resulting diff.
- [ ] T002 Add authenticated refresh request/outcome types and fixtures in
      `crates/cluster-protocol/src/lib.rs` (depends on T001).
- [ ] T003 [P] Add explicit secret-safe materialization and reload/bootstrap
      categories plus tests in `crates/executors/src/mcp_refresh.rs` (depends on
      T001).

## Layer 2: Worker refresh ownership

- [ ] T004 Refactor scoped MCP preparation and live `WorkerJob` state in
      `crates/worker/src/execution.rs` so the job retains its config target,
      profile adapter, refresh control, and per-execution claim (depends on
      T002, T003).
- [ ] T005 Implement worker-side validate -> atomic rematerialize -> Codex
      reload ordering and phase outcomes in `crates/worker/src/execution.rs`
      (depends on T004).
- [ ] T006 Wire the signed execution refresh route and safe error mapping in
      `crates/worker/src/worker_api.rs` (depends on T002, T005).
- [ ] T007 [P] Add the matching signed client operation and response decoding in
      `crates/services/src/services/cluster/client.rs` (depends on T002).

## Layer 3: Coordinator routing and result semantics

- [ ] T008 Extract one authoritative Codex profile/settings snapshot resolver
      for dispatch and refresh in `crates/local-deployment/src/container.rs`
      (depends on T001).
- [ ] T009 Route remote refresh by persisted execution/worker affinity and map
      worker outcomes into `McpRefreshCoordinator` generations in
      `crates/local-deployment/src/container.rs` (depends on T006, T007, T008).
- [ ] T010 Regenerate Rust-to-TypeScript contracts with the repository generator,
      updating `shared/types.ts` from source definitions (depends on T003).
- [ ] T011 [P] Update distinct safe refresh copy and status rendering tests in
      `packages/web-core/src/features/workspace-chat/ui/SessionChatBoxContainer.tsx`
      and adjacent test files if generated categories change (depends on T003).

## Layer 4: Regression and smoke coverage

- [ ] T012 Add worker tests for snapshot A -> B, added/updated/disabled/removed
      servers, section-only preservation, execution isolation, contention, and
      phase-specific secret-safe failures in `crates/worker/src/execution.rs` and
      `crates/worker/src/worker_api.rs` (depends on T005, T006).
- [ ] T013 Add a deterministic worker-side MCP initialize + `tools/list` smoke
      fixture under `crates/worker/tests/` using the refreshed scoped config
      (depends on T005).
- [ ] T014 [P] Add coordinator regression coverage for remote A -> B refresh,
      persisted worker routing, version skew, and conversation/session identity
      preservation in `crates/local-deployment/src/container.rs` (depends on
      T009).
- [ ] T015 [P] Complete UI coverage for pending, refreshed, partial, busy,
      unsupported, materialization failure, and reload/bootstrap failure in the
      existing `packages/web-core` test surface (depends on T010, T011).

## Layer 5: Verification and delivery artifacts

- [ ] T016 Run `pnpm install --frozen-lockfile`, repository formatting, focused
      Rust tests, worker integration/smoke tests, frontend tests/type checks, and
      generated-type checks; record any unrelated baseline failures.
- [ ] T017 Reconcile `SPEC.md`, `IMPLEMENTATION_PLAN.md`, and pipeline artifacts
      with the implemented behavior and final file set.
- [ ] T018 Run independent `codex review --uncommitted`, address every confirmed
      significant finding, and repeat focused verification/review until clean.
- [ ] T019 Distill reusable worker-scoped live-refresh knowledge into
      `docs/knowledge-base/`, update `docs/knowledge-base/INDEX.md`, and commit
      the knowledge-base change before handoff; if none emerged, record that
      explicitly.
