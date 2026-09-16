# Implementation plan — vk/1d23-vk-worker-output

1. Use workspace PRIOR_KNOWLEDGE.md to trace durable worker cursors, bounded replay retention, coordinator restart adoption, and output persistence.
2. Run workspace SpecKit command instructions in order, writing task artifacts to homelab/specs/vk/1d23-vk-worker-output/ and reaffirming applicable constitutions.
3. Reproduce the mismatch between acknowledged replay state and the coordinator's polling position; identify the smallest safe correction, including transcript recovery and acknowledgement ordering as necessary.
4. Install locked dependencies, implement regression coverage and correction, preserve explicit indeterminate outcomes for real gaps, and improve diagnostic context.
5. Run required formatting and focused Rust checks/tests; record environmental limits accurately.
6. Obtain independent Codex diff review, fix confirmed findings, and re-verify.
7. Update and commit reusable knowledge tagged with this task, then commit code/artifacts, open PRs against the repositories' base branches and merge after required checks.
