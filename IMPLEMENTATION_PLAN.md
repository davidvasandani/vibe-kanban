# Implementation plan — vk/5276-debug-unrecogniz

See `SPEC.md` and `specs/vk/5276-debug-unrecogniz/` for the full plan and tasks.

1. **Tests first** (`crates/local-deployment/src/container.rs`,
   `worker_event_tests`): add tests asserting that the classifier
   - passes a serialized `LogMsg` through unchanged;
   - maps every `cancellation_phase` value to `CancellationPhase`;
   - maps `worker_error` and `stream_error` to readable diagnostics with the reason verbatim;
   - maps unknown objects, invalid JSON, and non-string values to
     `Unrecognized worker event: <json>`.
2. **Classifier**: add `enum WorkerStructuredEvent { Log, CancellationPhase,
   Diagnostic }` and `fn classify_worker_structured(&str)` next to
   `push_worker_bytes`. Try `LogMsg` first, then match a single-key object with a string value.
3. **Wire in**: in the worker event poll loop, replace the
   `ExecutionEventPayload::Structured` arm:
   - Log → existing final-assistant tracking plus `store.push`;
   - CancellationPhase → `tracing::debug!` only;
   - Diagnostic → `store.push(LogMsg::Stderr(..))`.
   Remove the `push_stdout` fallback.
4. **Verify**: `cargo test -p local-deployment`,
   `cargo clippy -p local-deployment --all-targets`, `cargo fmt --all --check`.
5. **Review**: run a Codex review of the diff and iterate until it has no significant findings.
6. **Knowledge**: add a wiki page on the worker journal / agent-stream boundary and
   update the INDEX.
7. **Ship**: squash the WIP commit, open a PR against `main`, and merge it.

Status: steps 1–4 are done. The tests pass (48/48 in `local-deployment`), clippy is clean, and fmt is clean.
