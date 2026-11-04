# Implementation plan

1. Complete SpecKit constitution, feature specification, clarification, technical plan, tasks, and analysis before code changes. Use this task's feature pointer and Vibe Kanban's constitution; preserve unrelated historical specs.
2. Extend the encrypted organization credential payload and Settings request/view types with an optional App configuration. Preserve existing PAT payloads. Validate IDs and require a normalized private-key reference.
3. Add one App JWT/installation token resolver with bounded requests, installation-owner verification, effective-permission health, expiry-aware caching, concurrency control, and secret-safe errors. Reuse the existing 1Password resolver and row-bound storage.
4. Integrate server credential resolution and health. Preserve existing source attribution and check coverage semantics.
5. Implement command-time refresh for workspace gh and git, including workers and terminals, using the existing authenticated coordinator/worker execution infrastructure. Do not pass private keys to agents or introduce an unauthenticated token endpoint. Test refresh after expiry and configured failures.
6. Extend the Settings card for PAT/App selection and staged health, regenerate TypeScript definitions, and add focused component tests.
7. Document installation, permissions, migration, rotation, and deferred admin bypass.
8. Run fresh-worktree dependency installation, focused tests, type checks, repository formatting and lint/check commands. Resolve failures and run independent Codex review until significant findings are resolved.
9. Record reusable knowledge with the task id and commit it. Open a PR, observe checks, and merge when eligible. Report live acceptance separately from automated test results and identify missing App provisioning if any.
