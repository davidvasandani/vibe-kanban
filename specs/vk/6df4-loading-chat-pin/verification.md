# Verification: Resource-Aware Chat Loading

## Focused evidence

- `cargo test -p services services::container::tests -- --nocapture`
  - PASS: 17 tests, including same-execution single-flight, different-execution
    independence, canceled waiter, abandoned leader retry, task abortion, dead
    cell reclamation, leader-published sidecar replay, and optimistic cache
    replay.
  - Test execution time: 0.06 seconds after compilation.
- `cargo test -p services services::normalized_log_cache::tests -- --nocapture`
  - PASS: 8 tests covering patch materialization, replay equivalence, schema
    invalidation, truncation refusal, atomic write cleanup, and missing cache.
  - Test execution time: under 0.01 seconds after compilation.

The repository has no checked-in vendor JSONL conversation fixture suitable for
a representative cold-normalizer benchmark. Fabricating one would not exercise
the stateful vendor normalizer faithfully. The implementation instead records
ownership-wait and capacity-wait milliseconds plus retained/dropped counts at
runtime, allowing the clarified 2-second p95 escalation gate to be evaluated on
real histories after deployment. The deterministic tests measure the important
control outcomes without turning timing into a flaky assertion; the optimistic
cache test retains a one-second deadlock guard.

## WebSocket cancellation audit

`crates/server/src/routes/execution_processes.rs` races initial
`stream_normalized_logs` acquisition against inbound socket close. Once
acquired, `handle_normalized_logs_ws` selects stream output against inbound
close and drops the stream on disconnect. The service stream owns the
per-execution lease, global permit, and normalizer abort handles, so both
waiting acquisition and active replay remain cancellation-safe without a route
change.

## Repository verification

- `pnpm install --frozen-lockfile` — PASS.
- `pnpm run format` — PASS; Rust workspaces and all frontend packages formatted.
- `pnpm run check` — PASS; local-web, remote-web, web-core, UI, the main Rust
  workspace, and the remote Rust workspace all checked successfully.
- `cargo test -p server execution_processes -- --nocapture` — PASS (the name
  filter selected no tests, but compiled all server test targets and verified
  the unchanged route boundary).
- `cargo clippy -p services --all-targets --features qa-mode -- -D warnings` —
  PASS for the changed crate.
- `pnpm run lint` — frontend lint passed; workspace backend lint stopped on an
  unrelated pre-existing `clippy::too_many_arguments` error at
  `crates/server/src/routes/workspaces/create.rs:297` (8 arguments versus the
  lint limit of 7). That file is unchanged by this task. The changed services
  crate passes strict Clippy independently as recorded above.
- `git diff --check` — PASS.

## Latest-base verification

After merging the latest `origin/main` (`db45c627`) into the task branch, the
17-test `services::container::tests` suite passed again. A fresh independent
`codex review --base origin/main` reported no actionable correctness
regressions.

## Delivery

- Pull request: [#236](https://github.com/davidvasandani/vibe-kanban/pull/236)
- Base: `main` at `db45c627` when the branch was published.
- GitHub Actions: change detection, backend tests, remote tests, and backend
  schema checks passed; frontend checks were correctly skipped for this
  backend-only diff. The workspace Clippy job reproduced the unrelated
  pre-existing `too_many_arguments` finding in
  `crates/server/src/routes/workspaces/create.rs:297`; the changed services
  crate passes strict Clippy locally.
- The PR was merged after the scoped checks and independent review completed.
