# Implementation plan

1. Refresh SpecKit command task paths and review existing constitution.
2. Specify credit/usage exclusion and clarify positive evidence versus broad keyword matching.
3. Inspect normalized error types and real executor messages; reuse existing categories.
4. Add a pure full-entry classifier and tests in services/error_remediation.rs.
5. Read normalized entries once in the server consumer before guard reservation; skip classified failures and pass existing bounded messages to launch.
6. Install locked dependencies, format, run focused Rust tests and applicable checks.
7. Run independent Codex CLI review, resolve significant findings and reverify.
8. Update and commit remediation wiki knowledge and index; commit change, open PR and merge after checks.

Admission must inspect all untruncated ErrorMessage entries; normalized errors currently provide only SetupRequired and Other, so use explicit text signatures without schema changes.
