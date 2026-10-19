# Implementation plan — Debug VK Load

1. Follow workspace SpecKit commands using homelab/.specify/feature-dir; preserve the task-specific path.
2. Correlate the observed 20:10 UTC follow-up failure with worker placement and bounded runtime metrics. Record evidence and limits in the task research artifact.
3. Preserve admission thresholds and sticky ownership. Add recovery advice to pressure refusals and identify the assigned worker in coordinator dispatch errors and stored failure evidence.
4. Add focused regression coverage for error details; retain admission boundary/replay and scheduler tests. No automatic migration or prolonged request retries.
5. Install locked dependencies, format, run relevant Rust verification and document any environmental blockers.
6. Run independent Codex CLI review, resolve significant findings, and re-verify.
7. Record reusable findings in wiki topic/index with task vk/bdd8-debug-vk-load, commit, open PRs for the affected repositories, and merge after required checks pass.
