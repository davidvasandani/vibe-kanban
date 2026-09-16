# Implementation plan — vk/1d23-vk-worker-output

1. Use workspace PRIOR_KNOWLEDGE.md to trace durable worker cursors, bounded replay retention, coordinator restart adoption, and output persistence.
2. Run workspace SpecKit command instructions in order, writing task artifacts to homelab/specs/vk/1d23-vk-worker-output/ and reaffirming applicable constitutions.
3. Recover replay-gap outcomes only from identity-matching retained terminal evidence, leaving the output incomplete and missing events unacknowledged. Isolate scoped Codex SQLite indices while preserving transcripts/authentication and MCP refresh.
4. Install locked dependencies and test terminal evidence identity/state/sequence boundaries, eviction, private SQLite directories, source preservation, surviving transcripts and refresh. Preserve indeterminate outcomes without terminal evidence; include cursor boundaries in diagnostics.
5. Run required formatting and focused Rust checks/tests; record environmental limits accurately.
6. Obtain independent Codex diff review, fix confirmed findings, and re-verify.
7. Update and commit reusable knowledge tagged with this task, then commit code/artifacts, open PRs against the repositories' base branches and merge after required checks.
