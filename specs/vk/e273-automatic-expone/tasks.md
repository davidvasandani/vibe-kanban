# Tasks: Automatic exponential backoff for model-capacity errors

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes.

## Phase 1: Foundations
- [x] T001 [P] Policy module with unit tests in `crates/services/src/services/capacity_backoff.rs`; register it in `crates/services/src/services/mod.rs`
- [x] T002 [P] `CapacityBackoffConfig` and the `Config.capacity_backoff` field with a defaults test in `crates/services/src/services/config/versions/v8.rs`; re-export in `crates/services/src/services/config/mod.rs`
- [x] T003 [P] Migration `crates/db/migrations/20261118000000_capacity_retry_wakeups.sql`; fields, named-column `insert`, agent-only budget, `insert_capacity_retry`, `capacity_attempt_for_execution`, capacity `message`, and tests in `crates/db/src/models/wakeup.rs`

## Phase 2: Core
- [x] T004 Fix every `Wakeup { .. }` literal for the new fields in `crates/server/src/wakeups.rs`, `crates/server/src/routes/**`, `crates/server/src/issue_comment_delivery/tests.rs` (depends on T003)
- [x] T005 `capacity_retry_superseded` and sweep-time cancellation in `crates/server/src/wakeups.rs` (depends on T003)
- [x] T006 Supersession cancel and stopped-turn exemption in `crates/server/src/issue_comment_delivery.rs`; tests in `crates/server/src/issue_comment_delivery/tests.rs` (depends on T005)
- [x] T007 Consumer in `crates/server/src/capacity_backoff.rs` with tests; `pub mod` in `crates/server/src/lib.rs`; spawn in `crates/server/src/startup.rs`; pin in `crates/server/tests/startup_wiring.rs` (depends on T001, T002, T003)
- [x] T008 [P] Skip capacity failures in `crates/server/src/error_remediation.rs` (depends on T001)
- [x] T009 Declare `CapacityBackoffConfig` in `crates/server/src/bin/generate_types.rs` and regenerate `shared/types.ts` (depends on T002, T003)

## Phase 3: Frontend and docs
- [x] T010 [P] Capacity title in `packages/web-core/src/pages/workspaces/WakeupRow.tsx`; test in `packages/web-core/src/pages/workspaces/WakeupRow.test.tsx` (depends on T009)
- [x] T011 [P] Settings checkbox in `packages/web-core/src/shared/dialogs/settings/settings/GeneralSettingsSection.tsx`; strings in `packages/web-core/src/i18n/locales/{en,es,fr,ja,ko,zh-Hans,zh-Hant}/settings.json` (depends on T009)
- [x] T012 [P] Document the behavior and setting in `docs/settings/general.mdx`

## Phase 4: Validation
- [x] T013 `pnpm run format`, `pnpm run generate-types:check`, `pnpm run check`, `pnpm run lint`, and Rust tests for `db`, `services`, `server`; fix what fails (depends on all above)

<!--
Conventions:
- `T001` … task ids are stable and referenced by the dependency graph.
- `[P]` … parallel-safe (independent files). Omit for tasks that must be serial.
- `[ ]` / `[x]` … completion checkbox, toggled from the workbench.
-->
