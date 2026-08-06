# Implementation Plan: Shared HTTP Slack MCP

Task id: `967a-migrate-slack-mc`

## Dependency-ordered steps

1. Research the pinned fork's current HTTP transport contract from its tagged
   source/release and verify bind flags, endpoint path, health behavior, token
   environment, and multi-client/session behavior.
2. Refresh the project constitution and create the SpecKit feature directory.
   Convert this technical spec into user-focused requirements, clarify all
   decisions, then generate plan/research/contracts/tasks and analyze them.
3. Add a deployment contract to
   `homelab/modules/vibe-kanban-rebuild.nix`:
   - coordinator-only Slack MCP enablement and options for private bind address,
     port, worker source addresses, token runtime path/reference, and endpoint;
   - a pinned launcher package/service command using the existing fork artifact;
   - runtime credential resolution that cannot place the Slack token in the Nix
     store or agent environment;
   - a hardened, restartable systemd unit independent from Vibe Kanban health;
   - private source-scoped firewall rules that work with both NixOS firewall
     modes and preserve loopback.
4. Configure the think2 coordinator as the singleton owner and declare think3
   and think4 as permitted MCP consumers, using the existing private cluster
   addresses and secret-management conventions.
5. Change Vibe Kanban's bundled `slack` catalog entry to Streamable HTTP at the
   deployment-provided endpoint, with no stdio launcher or token placeholder.
   Introduce a narrowly scoped environment override if the catalog cannot safely
   hard-code a site-specific URL.
6. Add an exact historical-template migration for the shipped Slack stdio entry
   so existing installations converge to HTTP while custom same-name entries
   remain untouched.
7. Refactor Slack fork provenance checks so the pinned launcher/tag/digest remain
   audited even though the catalog no longer embeds the archive URL. Update
   executor-adapter and migration tests for Claude, Codex, Gemini, Grok, and
   Opencode HTTP shapes.
8. Update operator and user documentation with the private endpoint contract,
   systemd verification, MCP initialize/tools-list check, migration behavior,
   and secret-safe troubleshooting.
9. Verify in increasing scope:
   - focused executor unit tests and the ignored pinned-artifact audit;
   - Rust format/check for affected crates;
   - Nix syntax and module evaluation/assertion tests for coordinator and worker
     configurations, including generated service and firewall content;
   - repository project-context and documentation checks.
10. Run `/speckit.implement` against the dependency-ordered task list, marking
    each task complete only after its change and focused verification land.
11. Run an independent Codex CLI diff review, fix every confirmed significant
    finding, and repeat tests/review until no significant findings remain.
12. Distill reusable lessons into the appropriate project knowledge-base pages,
    tag them with `967a-migrate-slack-mc`, refresh indexes, and commit the
    knowledge-base update before handoff.

## Parallel layers

- After research and contracts are fixed, the Nix service work and Vibe Kanban
  catalog/migration work are independent implementation tracks.
- Documentation may proceed after both contracts stabilize.
- Focused Rust and Nix verification can run concurrently; final analysis and
  independent review wait for both.
