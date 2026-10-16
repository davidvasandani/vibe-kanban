# Implementation plan — vk/e831-remember-filter

1. Complete SpecKit constitution/specification/clarification/plan/tasks/analysis in order.
2. Add a dedicated board preference model with remembered per-user project/view settings and explicit user/site defaults. Preserve built-in Team/Personal defaults as the final fallback and exclude transient search from saved defaults.
3. Connect persistence to the existing scratch infrastructure, separating board records from the legacy globally shared UI scratch. Prevent hydration races and expose loading/save failures.
4. Wire the Kanban container and Filters dialog to the resolver and save/reset actions. Keep project-specific tags from filtering unrelated projects.
5. Add regression tests for precedence, reloads, user changes, empty overrides, and errors; run frontend checks/lint and required repository formatting.
6. Run independent Codex CLI review and fix significant findings.
7. Update and commit the wiki topic/index with this task ID, open a PR against the base branch, verify checks and merge.

Prior knowledge: ../PRIOR_KNOWLEDGE.md. Site-wide semantics will be finalized in clarification.
