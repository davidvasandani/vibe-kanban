# Tasks: Disable unused agents

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes.

## Phase 1: Storage (backend)
- [x] T001 Add `disabled: bool` to `ExecutorProfile`:
  - serde `default` plus `skip_serializing_if = Not::not`, with a doc comment
  - carry it in `merge_with_defaults` and `compute_overrides`
  - add `recommendation_candidates()` (skips disabled agents) and use it in
    `get_recommended_executor_profile`

  File: `crates/executors/src/profile.rs`
- [x] T002 [P] Add `disabled: false` to the test helper's `ExecutorProfile`
  literal in `crates/executors/src/env.rs` (depends on T001)
- [x] T003 [P] Add `disabled: false` to the `ExecutorProfile` literal in
  `crates/local-deployment/src/container.rs` (depends on T001)
- [x] T004 [P] Copy `disabled` from the source profile into the dispatched
  `ExecutorProfile` in `crates/worker/src/execution.rs` (depends on T001)
- [x] T005 Add Rust unit tests in `crates/executors/src/profile.rs`
  (depends on T001):
  - round-trip without becoming a variant
  - `false` is omitted
  - survives override computation and merge
  - re-enabling writes no override
  - recommendation candidates skip disabled agents
- [x] T006 Regenerate `shared/types.ts` (`pnpm run generate-types`)
  (depends on T001)

## Phase 2: Shared frontend resolver
- [x] T007 [P] Add `'disabled'` to `RESERVED_KEYS` in
  `packages/web-core/src/shared/lib/executor.ts` (depends on T006)
- [x] T008 [P] Create `packages/web-core/src/shared/lib/disabledAgents.ts`
  with `isAgentDisabled`, `filterEnabledAgents`, `setAgentDisabled` and
  `getDisableAgentBlocker` (depends on T006)
- [x] T009 [P] Add Vitest coverage in
  `packages/web-core/src/shared/lib/disabledAgents.test.ts` (depends on T008)

## Phase 3: Settings → Agents
- [x] T010 [P] Add the i18n keys `settings.agents.editor.disabled` and
  `settings.agents.visibility.{title,description,label,defaultLocked,lastLocked}`
  to every
  `packages/web-core/src/i18n/locales/{en,es,fr,ja,ko,zh-Hans,zh-Hant}/settings.json`
- [x] T011 Update
  `packages/web-core/src/shared/dialogs/settings/settings/AgentsSettingsSection.tsx`
  (depends on T007, T008, T010):
  - dim disabled rows and show the Disabled badge
  - add `AgentVisibilityCard` with the locked states
  - disable Make Default for disabled agents

## Phase 4: Pickers
- [x] T012 [P] In `packages/web-core/src/shared/hooks/useExecutorConfig.ts`,
  extract a pure `resolveEffectiveExecutor`. It skips disabled agents for
  last-used and config default, and options keep the effective agent
  (depends on T008)
- [x] T013 [P] Add Vitest coverage for `resolveEffectiveExecutor` in
  `packages/web-core/src/shared/hooks/useExecutorConfig.test.ts` (depends on
  T012)
- [x] T014 [P] Filter agents in
  `packages/web-core/src/shared/components/tasks/AgentSelector.tsx`, keeping
  the selected agent (depends on T008)
- [x] T015 [P] Filter the default-agent options and remediation executors in
  `packages/web-core/src/shared/dialogs/settings/settings/GeneralSettingsSection.tsx`,
  keeping their current values (depends on T008)
- [x] T016 [P] Filter the onboarding agent options in
  `packages/web-core/src/features/onboarding/ui/LandingPage.tsx`, keeping
  the selected agent (depends on T008)

## Phase 5: Validation
- [x] T017 Run `cargo test -p executors`, `cargo check --workspace`,
  `pnpm run generate-types:check`, `pnpm run check`, `pnpm run lint`,
  web-core Vitest, `scripts/check-i18n.sh` and `pnpm run format` (depends on
  T001–T016)
- [x] T018 Visual check of Settings → Agents and a picker in a running dev
  build, if feasible (depends on T017). Not feasible inside an agent turn,
  because background dev servers are reaped. Replaced by a rendered-DOM
  test. `AgentVisibilityCard` was extracted to
  `packages/web-core/src/shared/dialogs/settings/settings/AgentVisibilityCard.tsx`
  and covered by `AgentVisibilityCard.test.tsx` (toggle both ways, both lock
  reasons).

## Implementation notes
- `filterEnabledAgents` returns every agent when none is enabled, even when a
  `keep` value is present. The picker then offers a real choice instead of
  just the kept value (analyze warning, constitution L).
- The blocker and Make Default read the Settings page's local, unsaved
  profiles, so a pending disable is respected before Save.
- The existing `settings.agents.availability.*` keys hold the
  agent-availability check strings, so the new card uses
  `settings.agents.visibility.*`.
- Two test groups fail only on this agent host and are unrelated to this
  change. They pass with the variables unset:
  - `shared_mcp_config` Slack tests, which read `VIBE_KANBAN_SLACK_MCP_URL`
  - worker `dispatched_github_tokens_get_worker_local_routing`, which reads
    the host's `GIT_CONFIG_*`/`VK_GITHUB_PAT_*`


<!--
Conventions:
- `T001` … task ids are stable and referenced by the dependency graph.
- `[P]` … parallel-safe (independent files). Omit for tasks that must be serial.
- `[ ]` / `[x]` … completion checkbox, toggled from the workbench.
-->
