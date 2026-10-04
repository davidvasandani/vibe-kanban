# Tasks: ChatGPT custom-connector access to the Vibe Kanban MCP

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes. `vk:` paths are in the vibe-kanban repo; others are in homelab.

## Phase 1: Setup
- [x] T001 [P] Add migration `vk:crates/db/migrations/20261004000000_mcp_oauth.sql` (four tables plus indexes, per data-model.md)
- [x] T002 [P] Vendor `terragrunt/environments/cloudflare-tunnel-vibe-remote/openai-chatgpt-connectors.json` and add `scripts/refresh-openai-connector-ranges.sh`
- [x] T003 [P] Add `ip_lists` variable, `bypass_access_ip_lists` hostname field, `cloudflare_zero_trust_list` resource, `ip_list` include and preconditions in `terragrunt/modules/cloudflare-tunnel/main.tf`

## Phase 2: Core
- [x] T004 Implement the DB model `vk:crates/db/src/models/mcp_oauth.rs` and register it in `vk:crates/db/src/models/mod.rs` (depends on T001)
- [x] T005 [P] Implement `vk:crates/server/src/mcp_oauth/config.rs` (`VK_MCP_OAUTH_PUBLIC_URL` parsing) and `vk:crates/server/src/mcp_oauth/crypto.rs` (random tokens, hashing, PKCE)
- [x] T006 Implement handlers and routers in `vk:crates/server/src/mcp_oauth/mod.rs` and the consent and error HTML in `vk:crates/server/src/mcp_oauth/pages.rs` (depends on T004, T005)
- [x] T007 Wire the routers in `vk:crates/server/src/routes/mod.rs` and `vk:crates/server/src/lib.rs`. Make sure verify works without relay signing (depends on T006)
- [x] T008 [P] Configure vibe bypass paths and the OpenAI list in `terragrunt/environments/cloudflare-tunnel-vibe-remote/terragrunt.hcl` (depends on T002, T003)
- [x] T009 [P] Add the Caddy `handle /oauth/mcp*` forward_auth route in `hosts/think/think2.nix`, stripping any client-sent `Cf-Access-Jwt-Assertion`
- [x] T010 [P] Set coordinator `VK_MCP_OAUTH_PUBLIC_URL` in `modules/vibe-kanban-rebuild.nix`

## Phase 3: Validation
- [x] T011 [P] DB model tests in `vk:crates/db/src/models/mcp_oauth.rs` (`#[cfg(test)]`; code exchange, rotation, reuse, revoke, prune and cap) (depends on T004)
- [x] T012 [P] Server tests in `vk:crates/server/src/mcp_oauth/tests.rs` (metadata, DCR, authorize split, consent, PKCE, token errors, verify, disabled 404) (depends on T007)
- [x] T013 [P] Extend `ci/check-edge-exposure.sh` (ip-list paths count as restricted; vibe OAuth invariants; `/mcp`, `/oauth`, `/oauth/authorize` never bypassed) (depends on T008)
- [x] T014 [P] Docs (and mirror this SpecKit directory into `vk:specs/vk/50df-chatgpt-custom-m/`): `vk:crates/mcp/AGENTS.md`, `docs/vibe-kanban-mcp-deployment.md`, `docs/cloudflare-edge-exposure.md`
- [x] T015 Run verification: VK `pnpm run format`, `cargo clippy`, `cargo test -p db -p server mcp_oauth`, and the router test; homelab `bash ci/check-edge-exposure.sh`, `tofu validate` on the module, and `nix-instantiate --parse` / eval of touched Nix files (depends on T011–T014)
