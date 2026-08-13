# Tasks: Close Stale Execution Follow-up Gaps

**Plan**: `./plan.md`

Tasks are dependency ordered. `[P]` tasks touch independent files and may run in
parallel within their layer.

## Phase 1: Historical baseline and failing contracts

- [x] T001 Reconstruct per-file SpecKit ownership from git history and record the
  restore map in `specs/vk/c89d-address-fable-fo/artifact-recovery.md`.
- [x] T002 Restore the prior workspace-creation record under
  `specs/vk/5e1e-vk-workspace-cre/`, move PR #226's record to
  `specs/vk/3488-fix-stale-execut/`, and reconcile
  `specs/vk/a5f8-concat-repeating/` according to T001 (depends on T001).
- [x] T003 [P] Add failing provider/composer activity cases in
  `packages/web-core/src/shared/providers/ExecutionProcessesProvider.test.tsx`
  and the nearest `useWorkspaceExecution`/composer test.
- [x] T004 [P] Add deterministic subscribe-before-snapshot reduction and real
  lag failures in `crates/services/src/services/events/streams.rs` tests.
- [x] T005 [P] Add failing history/live ordering and lag tests in
  `crates/utils/src/msg_store.rs`.
- [x] T006 [P] Add failing initial-error, Ready-retention, and open-before-Ready
  backoff tests in
  `packages/web-core/src/shared/hooks/useJsonPatchWsStream.reconnect.test.tsx`.
- [x] T007 [P] Add a reserved-close relay regression in
  `packages/remote-web/src/shared/lib/relay/ws.test.ts`.
- [x] T008 Add deterministic missing/delayed Codex finalization tests in the
  owning executor/container modules after tracing the paths in
  `crates/executors/src/executors/codex/`,
  `crates/services/src/services/container.rs`, and
  `crates/local-deployment/src/container.rs`.

## Phase 2: Shared runtime truth and stream authority

- [x] T009 Extract the neutral running-attempt predicate and use it from
  `packages/web-core/src/shared/hooks/useExecutionProcesses.ts` and
  `packages/web-core/src/shared/providers/ExecutionProcessesProvider.tsx`
  (depends on T003).
- [x] T010 Extract subscribe-first/lag-fatal stream construction in
  `crates/services/src/services/events/streams.rs` and apply it to execution,
  scratch, workspace, and browser-session streams (depends on T004).
- [x] T011 Make `MsgStore::history_plus_stream` subscribe before history and
  return lag as an authority error in `crates/utils/src/msg_store.rs`; adjust
  direct consumers under `crates/executors`, `crates/services`, and
  `crates/deployment` as required (depends on T005).
- [x] T012 Route authority errors through retryable error-close handling and
  add a real execution-process WebSocket close test in
  `crates/server/src/routes/execution_processes.rs` (depends on T010).
- [x] T013 Add focused sibling-stream state/filter tests in
  `crates/services/src/services/events/streams.rs` and document any proven
  exemption in `specs/vk/c89d-address-fable-fo/research.md` (depends on T010,
  T011).

## Phase 3: Client recovery and relay diagnostics

- [x] T014 Separate initial allocation, endpoint Ready, and unhealthy retry
  counters in `packages/web-core/src/shared/hooks/useJsonPatchWsStream.ts`; reset
  backoff only on Ready and retain prior valid state (depends on T006).
- [x] T015 Preserve decoded close code/reason through a browser-legal raw close
  and emit one consumer CloseEvent in
  `packages/remote-web/src/shared/lib/relay/ws.ts` (depends on T007).
- [x] T016 Run focused frontend hook/provider/relay tests and update tests for
  any observed runtime boundary mismatch (depends on T009, T014, T015).

## Phase 4: Bounded execution finalization

- [x] T017 Document every pre-`update_completion` error/early-return and the
  local/worker evidence source in
  `specs/vk/c89d-address-fable-fo/finalization-audit.md` (depends on T008).
- [x] T018 Add the final-assistant-output reconciliation trigger and testable
  45-second registration in the normalization/container boundary identified by
  T017 (depends on T017).
- [x] T019 Implement owner-specific local and cluster liveness probes plus the
  shared truthful classification decision in
  `crates/local-deployment/src/container.rs` and relevant
  `crates/services/src/services/cluster/` modules (depends on T018).
- [x] T020 Add bounded terminal-write retry/diagnostics and preserve existing WIP
  ordering in `crates/services/src/services/container.rs` and
  `crates/local-deployment/src/container.rs` (depends on T019).
- [x] T021 Complete paused-time tests for delayed/lost/interrupted finalization,
  live/dead local ownership, valid/expired worker lease, replay gap, and DB write
  failure in the modules changed by T018–T020 (depends on T020).
- [x] T022 Add an authoritative terminal-stream-to-provider/composer regression
  proving final output plus missing finalization returns Send without refresh in
  the nearest backend/frontend integration harness (depends on T009, T012,
  T021).

## Phase 5: Artifact collision prevention

- [x] T023 Locate and fix the source of hard-coded `a5f8-concat-repeating`
  SpecKit command paths under `.claude/commands/`, pipeline assets, and their
  generating Rust/TypeScript source so paths derive from current task identity
  (depends on T002).
- [x] T024 Add different-owner refusal and same-owner refresh tests in the
  pipeline/SpecKit generator's existing test module, and update all checked-in
  command references to isolated directories (depends on T023).

## Phase 6: Verification, review, and delivery

- [x] T025 Run focused Rust and frontend suites for all changed contracts and
  record commands/results in `specs/vk/c89d-address-fable-fo/verification.md`
  (depends on T013, T016, T022, T024).
- [x] T026 Run `pnpm install --frozen-lockfile` if needed, `pnpm run format`,
  generated-type checks as applicable, `pnpm run check`, `pnpm run lint`, and
  applicable broad Rust tests; record results in
  `specs/vk/c89d-address-fable-fo/verification.md` (depends on T025).
- [x] T027 Run independent `codex review`, address confirmed significant
  findings, rerun affected verification, and repeat until clean; write
  `specs/vk/c89d-address-fable-fo/review.md` (depends on T026).
- [x] T028 Update reusable project knowledge in `docs/knowledge-base/`, tag it
  `c89d-address-fable-fo`, refresh `docs/knowledge-base/INDEX.md`, and commit the
  knowledge-base work (depends on T027).
- [x] T029 Commit remaining scoped changes, push the task branch, open a PR
  against the current base, pass required checks, and merge it (depends on
  T028).
