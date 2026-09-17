# Tasks: Issue and workspace lifecycle

## Setup
- [x] T001 Install locked dependencies with pnpm; preserve dependency manifests.
## Core (independent backend layers)
- [x] T002 [P] Implement transactional reactivation in `crates/remote/src/db/workspaces.rs` with transition regression coverage.
- [x] T003 [P] Add activation hook in `crates/services/src/services/container.rs`, implement via `crates/services/src/services/remote_sync.rs` and `crates/local-deployment/src/container.rs`, and call from `crates/server/src/routes/sessions/queue.rs`; correct completion sync snapshot.
## Validation (depends on T002/T003)
- [ ] T004 Run focused regression tests, backend checks and `pnpm run format`; record results in `specs/vk/e464-issue-and-worksp/validation.md`.
- [x] T005 Independently review with Codex CLI, fix findings and reverify.
## Delivery
- [x] T006 Record reusable knowledge in `wiki/issue-workspace-lifecycle.md` and `wiki/INDEX.md`, commit knowledge.
- [ ] T007 Open PR against main and merge after checks.
