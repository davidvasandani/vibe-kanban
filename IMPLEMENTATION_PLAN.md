# Implementation plan — vk/b0d4-env-vars-value-f

1. Follow the workspace SpecKit commands in order; preserve their homelab paths.
2. Inspect the existing CLI catalog, environment fetch/merge, terminal and worker
   consumers, and shared settings UI. Verify the 1Password CLI contract.
3. Add a bounded asynchronous resolver using explicit service-account auth and
   host/app-installed CLI discovery, no shell, no raw provider errors. Retain
   exact literal/secret bytes, atomic map replacement, and fresh per-launch reads.
4. Make the shared workspace env resolver fallible for reference failures and
   propagate errors through local, worker-dispatch, and terminal launch paths.
5. Explain literal and op:// values in organization settings and docs.
6. Add deterministic fake-provider tests for precedence, failures, redaction,
   cancellation/timeouts, and exact values; run formatting and relevant checks.
7. Run independent Codex CLI review; fix findings and re-verify.
8. Update and commit the knowledge base with task tags and index entry.
9. Commit changes, open PRs against each repository's base as needed for mandated
   SpecKit artifacts, wait for required checks, and merge.

Prior knowledge: ../PRIOR_KNOWLEDGE.md (workspace environment inheritance and fail-loud boundaries). No new package dependency expected.
