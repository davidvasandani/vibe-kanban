# Tasks: Verified Slack MCP installation

**Plan**: `./plan.md`

Tasks are dependency-ordered. Tasks marked **[P]** touch independent files and
may be completed together within their layer.

## Phase 1: Baseline

- [x] T001 Incorporate predecessor commit `2e4b77aa`, preserving this task's
  `SPEC.md`, `PRIOR_KNOWLEDGE.md`, `IMPLEMENTATION_PLAN.md`,
  `.specify/memory/constitution.md`, and
  `specs/vk/95e9-close-the-unveri/**`.
- [x] T002 Resolve the predecessor/current constitution collision in
  `.specify/memory/constitution.md`, preserving principles XV and XVI (depends
  on T001).
- [x] T003 Run the predecessor Slack catalogue tests without changing files
  (depends on T001).

## Phase 2: Detection and notification

- [x] T004 Change the pinned-artifact audit to daily and add least-privilege,
  failure-only, deduplicated GitHub issue notification in
  `.github/workflows/pinned-artifacts.yml` (depends on T001).
- [x] T005 [P] Update the GitHub-release Renovate reviewer note so it accurately
  calls the outer digest a scheduled audit in `renovate.json` (depends on T001).
- [x] T006 [P] Add the temporary detect-only decision, residual threat,
  notification path, and npm reopening trigger to
  `docs/knowledge-base/forked-mcp-server-packaging.md` (depends on T001).
- [x] T007 [P] Update the user-facing delivery and verification posture in
  `docs/integrations/mcp-server-configuration.mdx` (depends on T001).

## Phase 3: Tests and repository consistency

- [x] T008 Ensure the Slack pin, adapter, and network digest tests in
  `crates/executors/src/mcp_config.rs` still express the unchanged GitHub source
  and audit-only outer digest; change comments/assertions only if inaccurate
  (depends on T001, T004).
- [x] T009 [P] Tag the reusable decision knowledge with
  `95e9-close-the-unveri` and refresh
  `docs/knowledge-base/INDEX.md` (depends on T006).
- [x] T010 Validate workflow YAML and `renovate.json` without changing tracked
  files (depends on T004, T005).

## Phase 4: Runtime verification

- [x] T011 Install locked workspace dependencies with
  `pnpm install --frozen-lockfile` (depends on Phase 2).
- [x] T012 [P] Run focused executor Slack catalogue and adapter tests (depends
  on T003, T008).
- [x] T013 [P] Run the ignored published-launcher digest audit (depends on T004,
  T008).
- [x] T014 [P] Run a clean npm-cache and absent-launcher-cache MCP handshake;
  confirm `attachment_get_data` in `tools/list`, then call it with a real Slack
  attachment if credentials/fixture are available (depends on T001). Re-run
  with the authorised 1Password-provided token and fresh npm/launcher caches:
  `attachment_get_data` appeared in `tools/list`, and retrieving fixture
  `F0BJX4Y3N5A` returned one non-empty content block (36,876 payload
  characters). No token or attachment content was logged.
- [x] T015 Run `pnpm run format` and proportionate repository checks (depends on
  T011 and all file-changing tasks).

## Phase 5: Review and knowledge capture

- [x] T016 Run independent Codex review on the complete diff and record findings
  (depends on T015).
- [x] T017 Address confirmed findings in the affected files and rerun relevant
  verification; repeat T016 until no significant findings remain (depends on
  T016).
- [x] T018 Commit the final reusable knowledge-base update in
  `docs/knowledge-base/forked-mcp-server-packaging.md` and
  `docs/knowledge-base/INDEX.md` before task handoff (depends on T009, T017).

## Dependency layers

- Layer A: T001
- Layer B: T002, T003
- Layer C: T004, T005, T006, T007
- Layer D: T008, T009, T010
- Layer E: T011, T012, T013, T014
- Layer F: T015
- Layer G: T016, T017
- Layer H: T018
