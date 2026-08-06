# Tasks: Shared HTTP Slack MCP

**Plan**: `./plan.md`

Tasks are dependency ordered. Tasks marked **[P]** touch independent files and
may run in parallel within their layer.

## Pre-implementation gate

- [x] T000 Run `/speckit.analyze`, resolve its planning gaps, and confirm the
      task list is constitution-compliant before source implementation.

## Layer 1: Product and deployment contracts

- [x] T001 Add the configurable Slack HTTP catalog override and update the
      canonical bundled definition in
      `vibe-kanban/crates/executors/default_mcp.json` and
      `vibe-kanban/crates/executors/src/mcp_config.rs`.
- [x] T002 [P] Add coordinator-only Slack MCP options, validation, pinned
      launcher service, runtime credential loading, and private firewall policy
      in `homelab/modules/vibe-kanban-rebuild.nix`.
- [x] T003 Wire the private endpoint into coordinator/worker runtime
      environments and enable the singleton on think2 in
      `homelab/hosts/think/think2.nix`,
      `homelab/hosts/think/think3.nix`, and
      `homelab/hosts/think/think4.nix` (depends on T001, T002).

## Layer 2: Migration and integrity

- [x] T004 Extend exact historical Slack template recognition and token-dropping
      HTTP reconciliation in
      `vibe-kanban/crates/executors/src/shared_mcp_config.rs` (depends on T001).
- [x] T005 Add/update catalog adapter, override, exact migration, custom-entry,
      idempotence, and pinned-artifact tests in
      `vibe-kanban/crates/executors/src/mcp_config.rs` and
      `vibe-kanban/crates/executors/src/shared_mcp_config.rs` (depends on T001,
      T004).
- [x] T006 [P] Add Nix module evaluation coverage for role validation, runtime
      credential/service command, endpoint environment, and dual firewall
      output in `homelab/tests/vibe-kanban-cluster.nix` (depends on T002, T003).

## Layer 3: Documentation and focused verification

- [x] T007 [P] Update Vibe Kanban user/operator MCP documentation in
      `vibe-kanban/docs/integrations/mcp-server-configuration.mdx` and relevant
      knowledge links without recording new task knowledge yet (depends on T001,
      T004).
- [x] T008 [P] Add the homelab Slack MCP deployment/runbook documentation under
      `homelab/docs/` and update `homelab/project-context.json` only if its
      authoritative Vibe Kanban mapping needs a docs pointer (depends on T002,
      T003).
- [x] T009 Run focused Rust format/tests/checks, the ignored launcher digest
      audit, Nix parse/evaluation/script checks, and project-context validation;
      fix failures in affected files (depends on T005, T006, T007, T008).

## Layer 4: Final controls

- [x] T010 Run independent Codex CLI diff review; address confirmed significant
      findings and repeat T009/review until clean (depends on T009).
- [x] T011 Distill shipped reusable knowledge, tag pages with
      `967a-migrate-slack-mc`, refresh both relevant knowledge-base indexes,
      and commit the knowledge-base changes (depends on T010).
