# Implementation plan — session step timestamps

1. Trace live and historical normalization and identify where durable event times can be preserved without stamping replay time.
2. Apply the project constitution and feature specification; resolve unknown-time behavior explicitly.
3. Implement shared timestamp propagation at the appropriate backend boundary, preserving entry identity on updates and existing source timestamps.
4. Remove misleading per-step execution-start fallback for legacy events without trustworthy times; retain explicit timestamps on synthetic prompt/script rows.
5. Add regressions for distinct steps, replacement stability, replay, missing timestamps, and grouped display selection.
6. Install locked dependencies, run targeted tests, type/lint checks and required formatting. Record environment limits accurately.
7. Run independent Codex diff review and address significant findings.
8. Update and commit topic knowledge, open a PR against the workspace base branch, verify checks, and merge.

Source knowledge: workspace `PRIOR_KNOWLEDGE.md`; repository copy retained for review.
