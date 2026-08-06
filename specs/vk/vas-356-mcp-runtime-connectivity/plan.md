# Technical Plan: Cluster-safe MCP runtime connectivity

## Design

The worker unit already owns the authoritative coordinator URL. Add
`VIBE_BACKEND_URL` beside `VK_WORKER_COORDINATOR_URL`, using the same
`stringOrEmpty cfg.worker.coordinatorUrl` expression. All executor children
inherit the worker environment, and `vibe-kanban-mcp` gives
`VIBE_BACKEND_URL` highest resolution priority.

On think1, replace the combined accept with two accepts before the existing
drop:

- think1 + think2 may reach 8189, 8190, and 3410;
- think3 + think4 may reach only 3410.

This preserves the dedicated base-chain enforcement required on think1, where
the general NixOS firewall is disabled.

## Verification

- Evaluate think3 and think4 worker unit environments.
- Evaluate think1 `vibe_remote_origins` table content and assert exact rules.
- Run homelab project-context and focused Nix checks where available.
- Run formatting/diff checks in both repositories.
- After deployment, initialize both MCP servers from a worker and enumerate
  their tools.

## Rollout

Merge the homelab Nix change after the Vibe repository documentation/spec commit
is available. Deploy think1, think3, and think4 through the existing homelab
rebuild mechanism. Existing Codex sessions then use the normal MCP refresh flow.
