# Tasks

Plan: ./plan.md

- [x] T001 Retain owner-bound gateway identity in OAuth start, PendingFlow, both completion paths, and exchange persistence (`crates/server/src/routes/mcp_auth.rs`).
- [x] T002 Add identity and assignment regression tests in `crates/server/src/routes/mcp_auth.rs` (depends T001).
- [ ] T003 Install locked dependencies, run formatting and targeted Rust verification; record results in `specs/vk/e89d-debug-atlassian/validation.md` (depends T002).
- [x] T004 Run independent Codex CLI diff review and address confirmed findings (depends T003).
- [ ] T005 Update OAuth knowledge topic and index, commit documentation and fix (depends T004).
- [ ] T006 Open and merge PR against verified base after required checks (depends T005).

The implementation and tests share one file, so no implementation tasks are parallel-safe.
