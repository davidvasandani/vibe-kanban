# Implementation plan — vk/a63c-don-t-obfuscate

1. Apply the workspace SpecKit commands in their specified order. Their exact
   artifact paths are in homelab; only Vibe Kanban task documentation belongs there.
2. In `packages/web-core/src/shared/dialogs/settings/settings/OrganizationEnvVarsCard.tsx`,
   derive both input types from the draft's exact `op://` prefix.
3. Add an adjacent rendered-DOM regression test for add/edit visibility,
   prefix removal, non-reference values, exact submitted values, and saved-row
   redaction. Use existing React/Vitest/jsdom tooling without dependencies.
4. Install frozen dependencies, run targeted tests, frontend checks/lint, and
   repository formatting. Record any environment or baseline check failures.
5. Run independent Codex diff review and address significant findings.
6. Update the environment-inheritance knowledge page and index, tagged with
   the task ID; commit it. Open and merge task PRs against repository base branches.
