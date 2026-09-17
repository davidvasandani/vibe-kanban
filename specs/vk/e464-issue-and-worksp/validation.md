# Validation
- `pnpm install --frozen-lockfile`: passed.
- `pnpm run format`: passed.
- `git diff --check`: passed.
- Targeted backend check: running. Remote `cargo check --manifest-path crates/remote/Cargo.toml`: passed.
- Remote lifecycle tests: running against an isolated temporary Postgres instance.
- Direct follow-up calls ContainerService::start_execution, which calls activate_workspace before dispatch (ArchiveScript excluded). Queue acceptance publishes its message before calling the same activation hook. Both local and remote effects use established persistence/sync APIs.

Independent review pass 1 found a queue handoff race introduced by awaiting remote activation before queue insertion. Fixed by publishing the message first so finalization can claim it during synchronization. Review will be repeated.

Remote activation sync is spawned using the established best-effort pattern, keeping network latency outside execution startup and request cancellation. A mock HTTP test verifies activation persistence and unarchive PATCHs even when already active locally.

Independent Codex review pass 2: “No actionable defects were identified in the current changes.” Reviewed the asynchronous sync helper, transaction, and queue ordering.
