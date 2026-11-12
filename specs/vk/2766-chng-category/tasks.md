# Tasks: CHNG category

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes.

## Phase 1: Storage
- [x] T001 [P] Migration `crates/db/migrations/20261108000000_chng_settings.sql`
- [x] T002 [P] Row model `crates/db/src/models/chng_settings.rs` + `crates/db/src/models/mod.rs`
- [x] T003 [P] Host key path `crates/utils/src/assets.rs` (`chng_settings_key_path`)

## Phase 2: Backend service + routes
- [x] T004 Service `crates/services/src/services/chng.rs` + `services/mod.rs`: settings view/update, URL/secret rules, Jira search, Hashmal lookup, unit tests (depends on T001–T003)
- [x] T005 Routes `crates/server/src/routes/chng.rs`, `routes/mod.rs`, error mapping `crates/server/src/error.rs` (depends on T004)
- [x] T006 Type export `crates/server/src/bin/generate_types.rs` → `shared/types.ts` (depends on T004)

## Phase 3: Frontend data + navigation
- [x] T007 [P] `packages/web-core/src/shared/lib/api.ts` `chngApi`; `packages/web-core/src/shared/lib/machineClient.ts` settings methods (depends on T006)
- [x] T008 [P] `packages/web-core/src/shared/lib/chngPrompt.ts` + `chngPrompt.test.ts` (depends on T006)
- [x] T009 [P] Rail: `packages/ui/src/components/AppBar.tsx`, `packages/web-core/src/shared/components/ui-new/AppBar.test.tsx`
- [x] T010 [P] Navigation: `packages/web-core/src/shared/lib/routes/appNavigation.ts`, `packages/local-web/src/app/navigation/AppNavigation.ts`, `packages/local-web/src/routes/_app.chng.tsx`, `packages/local-web/src/routeTree.gen.ts`

## Phase 4: Frontend surfaces
- [x] T011 CHNG page + dialog `packages/web-core/src/pages/chng/ChngPage.tsx`, `ChngStartSessionDialog.tsx` (depends on T007, T008, T010)
- [x] T012 Layout wiring `packages/web-core/src/pages/root/SharedAppLayout.tsx` (rail + mobile drawer) (depends on T009, T010)
- [x] T013 Settings `packages/web-core/src/shared/dialogs/settings/settings/ChngSettingsSection.tsx`, `SettingsRegistry.tsx`, locale `settings.json` nav labels (depends on T007)

## Phase 5: Hashmal (separate repo)
- [x] T014 [P] `apps/hashmal/backend/mcp_server/tools.py`, `tests.py`, `README.md` in `sweetgreen/platform-ops`; open PR (not merged) — sweetgreen/platform-ops#350

## Phase 6: Validation
- [x] T015 Rust tests, `generate-types:check`, `pnpm run check`, `pnpm run lint`, Vitest, `pnpm run format`
- [x] T016 [P] Wiki page `wiki/chng-category.md` + `wiki/INDEX.md` (stage 12)
