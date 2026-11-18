# Tasks (/speckit.tasks)

Plan: ./plan.md. Tasks are sequential because admission depends on the classifier, tests share its file, and documentation follows verification.

- [x] T001 Install locked dependencies with pnpm install --frozen-lockfile.
- [x] T002 Add credit/usage classifier and table-driven boundary tests in crates/services/src/services/error_remediation.rs.
- [x] T003 Wire early exclusion and single evidence read in crates/server/src/error_remediation.rs (depends T002).
- [x] T003a Preserve failed Result evidence in crates/executors/src/executors/claude.rs and update crates/executors/src/executors/qa_mock.rs; add normalizer-to-admission coverage in services/error_remediation.rs (independent review correction).
- [x] T004 Format, run focused tests and compile affected server; address failures (depends T001–T003).
- [x] T005 Independently review diff using Codex CLI and address confirmed findings (depends T004).
- [x] T006 Update wiki/auto-error-remediation.md and wiki/INDEX.md with task provenance and commit knowledge (depends T005).
- [ ] T007 Commit changes, create PR against base, verify checks and merge (depends T006).
