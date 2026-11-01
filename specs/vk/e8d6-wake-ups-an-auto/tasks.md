# Tasks

Applied `/speckit.tasks`. Dependencies flow top to bottom; independent file groups may run together without delegated agents.

- [x] T001 Persist stable archive intent and acknowledgement in `crates/db/migrations/`, `crates/db/src/models/workspace.rs`, and remote archive provenance in `crates/remote/`.
- [x] T002 Route all automatic archive sources and deferred retries through admission in `crates/services/src/services/{container,pr_monitor}.rs` and `crates/server/src/routes/workspaces/{core,git,pr}.rs`. Depends T001.
- [x] T003 Make wake-up acceptance and reactivation atomic in `crates/db/src/models/wakeup.rs` and improve `crates/server/src/wakeups.rs`. Depends T001.
- [x] T004 Propagate exact origin in `crates/local-deployment/src/container.rs`; resolve it in `crates/mcp/src/task_server/tools/pollers.rs`. Independent of T001–T003.
- [x] T005 Expose provenance in workspace API, MCP list and shared workspace UI. Depends T001.
- [ ] T006 Add regression tests beside changed modules; format and run relevant verification. Depends T002–T005.
- [x] T007 Run independent Codex diff review, address confirmed findings and reverify.
- [x] T008 Update `wiki/durable-session-wakeups.md`, `wiki/issue-workspace-lifecycle.md` and `wiki/INDEX.md`; commit knowledge.
- [ ] T009 Open PR, verify checks and merge against the repository base branch.
