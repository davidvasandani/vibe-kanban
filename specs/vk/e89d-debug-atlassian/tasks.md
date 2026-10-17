# Tasks

Plan: ./plan.md

- [x] T001 Retain owner-bound gateway identity in OAuth start, PendingFlow, both completion paths, and exchange persistence (`crates/server/src/routes/mcp_auth.rs`).
- [x] T002 Add identity and assignment regression tests in `crates/server/src/routes/mcp_auth.rs` (depends T001).
- [x] T003 Install locked dependencies, run formatting and targeted Rust verification; record results in `specs/vk/e89d-debug-atlassian/validation.md` (depends T002).
- [x] T004 Run independent Codex CLI diff review and address confirmed findings (depends T003).
- [x] T005 Update OAuth knowledge topic and index, commit documentation and fix (depends T004).
- [x] T006 Open PR #301 against verified base `main` and verify required checks (depends T005).

Final delivery gate: merge PR #301 after the documentation-only update checks pass; the GitHub merge record is authoritative.

The implementation and tests share one file, so no implementation tasks are parallel-safe.
