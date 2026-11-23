# Tasks

Applied `/speckit.tasks`.

## Layer 1 (independent)
- [x] T001 [P] Add gateway provider contract and integrate target identity/discovery/selection/execution in crates/services/src/services/reauth/{gateway,mod}.rs. Update Settings auth grouping and tests; generate shared types.
- [x] T002 [P] Implement server gateway provider, expiry persistence, refresh/browser repair and error annotation in crates/server/src/mcp_gateway/{mod,proxy,reauth}.rs; register provider at startup.
- [x] T003 [P] Fix scoped cancel_wakeup route extraction and add regression coverage in crates/server/src/routes and crates/mcp/src/task_server/tools. Update MCP recovery instructions for gateway targets.

## Layer 2
- [ ] T004 Integrate contracts, add end-to-end forced-expiry/stable-bearer fixtures and run focused checks plus repository format/check requirements.
- [x] T005 Run independent Codex diff review; resolve significant findings and reverify.
- [x] T006 Record reusable gateway renewal knowledge in wiki and index; commit.
- [ ] T007 Open PR against main, satisfy required checks, merge and report verification limits.
