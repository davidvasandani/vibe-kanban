# Implementation Plan: Refresh ChatGPT Model Catalog

1. Establish the SpecKit constitution and generate the feature's detailed
   specification artifacts under `specs/vk/094a-update-chatgpt-m/`.
2. Clarify the exact catalog, order, model identifiers, and reasoning-effort
   assignments from the supplied ChatGPT reference and repository precedent.
3. Produce the SpecKit technical plan and dependency-ordered task list, then
   analyze those artifacts for gaps and constitution conflicts.
4. Replace the Codex executor's advertised model list with the six current
   choices, reusing the existing reasoning-option builders where their
   capability levels match.
5. Add focused regression coverage for exact model ordering, labels, and
   reasoning choices.
6. Format and run relevant Rust tests/checks plus repository-required
   verification.
7. Run an independent Codex diff review, fix confirmed findings, and repeat
   verification/review until no significant findings remain.
8. Add reusable model-catalog knowledge to the wiki and refresh its index.
9. Commit the implementation and knowledge, use Vibe Kanban's Create PR flow,
   wait for CI, and merge the PR into the base branch.
