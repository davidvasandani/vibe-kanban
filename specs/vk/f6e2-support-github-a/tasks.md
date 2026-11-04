# Tasks

- [x] T001 Add validated App payload and backwards-compatible CRUD in `crates/services/src/services/github_owner_tokens.rs`.
- [x] T002 Implement signing, installation verification, token caching and health in `crates/services/src/services/github_app.rs`, `crates/services/src/services/mod.rs`, `crates/services/Cargo.toml`; wire `github_credentials.rs`. Depends T001.
- [x] T003 Implement scoped refresh capabilities and broker in `github_app.rs`, `crates/server/src/routes/github_owner_tokens.rs`, `crates/server/src/routes/mod.rs`; connect launch in `github_owner_tokens.rs`. Depends T002.
- [x] T004 Add command-time broker resolution in `crates/utils/src/github_auth.rs`, `crates/utils/src/github_auth/gh-shim.sh`; use the spawning worker's existing `VK_WORKER_COORDINATOR_URL` at the shared routing boundary. Depends T003.
- [x] T005 [P] Extend `packages/web-core/src/shared/dialogs/settings/settings/GitHubOwnerTokensCard.tsx` and test file, `packages/web-core/src/shared/lib/machineClient.ts` health API; register types in `crates/server/src/bin/generate_types.rs` and regenerate `shared/types.ts`. Depends T002.
- [x] T006 [P] Document setup and permissions in `docs/integrations/github-integration.mdx` following `docs/AGENTS.md`. Depends T002.
- [x] T007 [P] Validate regression tests beside resolver/storage/broker/routing implementation; verify Rust, frontend, generated types and formatting. Depends T004-T006. Focused checks pass: 26 service tests, 12 routing tests, 872 web-core tests plus 8 Settings tests; full check, backend Clippy, frontend lint, formatting and generated-type checks passed.
- [x] T008 [P] Run independent Codex review, address significant findings and reverify. Depends T004-T006. Source review can run alongside T007 read-only validation; wait for both before readiness.
- [x] T009 Record shipped knowledge in `wiki/github-owner-token-routing.md` and `wiki/INDEX.md`, tagged with task id; commit. Depends T007 and T008.
- [ ] T010 Open PR, observe CI and merge. Report live acceptance evidence or explicit missing prerequisites. Depends T009.
