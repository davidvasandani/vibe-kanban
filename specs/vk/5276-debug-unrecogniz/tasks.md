# Tasks: Keep worker control metadata out of the agent chat

**Plan**: `./plan.md`

All code changes are in one file, so the code tasks are sequential. `[P]` marks work
that touches independent files.

## Layer 1 — Classifier (tests first)
- [x] T001 Add unit tests for `classify_worker_structured` covering LogMsg
      passthrough, `cancellation_phase`, `worker_error`, `stream_error`, an
      unknown object, and invalid JSON. Tests go in the existing
      `push_worker_bytes` test module.
      File: `crates/local-deployment/src/container.rs`
- [x] T002 Add the `WorkerStructuredEvent` enum and the
      `classify_worker_structured` function next to `push_worker_bytes` so that
      T001 passes. Depends on: T001.
      File: `crates/local-deployment/src/container.rs`

## Layer 2 — Wire into the poll loop
- [x] T003 Replace the `ExecutionEventPayload::Structured` arm of the worker
      event poll loop with a match on `classify_worker_structured`: Log →
      existing behaviour; CancellationPhase → `tracing::debug!`; Diagnostic →
      `LogMsg::Stderr`. Depends on: T002.
      File: `crates/local-deployment/src/container.rs`

## Layer 3 — Verify and document
- [x] T004 Run `cargo test -p local-deployment` (the classifier tests plus the existing
      worker tests) and `cargo clippy -p local-deployment`. Depends on: T003.
- [x] T005 [P] Run `pnpm run format` (at minimum `cargo fmt --all`). Depends on: T003.
