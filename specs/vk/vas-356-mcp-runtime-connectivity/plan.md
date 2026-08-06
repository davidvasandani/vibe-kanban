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

## Follow-up design: dispatch-time MCP snapshot

The coordinator reads the selected coding agent's native MCP section through
the existing adapter and attaches only that server map plus the base executor
identity to signed `ExecutionDispatch`. The optional snapshot has a 1 MiB
canonical JSON bound and participates in request digest material.

Reusable extract-and-replace helpers live in
`crates/executors/src/mcp_config.rs`. Replacement reads the worker native
config/template, changes only the adapter's MCP path, and uses the existing
atomic writer. Worker admission validates identity and size, then materializes
before creating or spawning the job. Absence retains backward compatibility;
invalid supplied state fails closed.

After synchronization deploys, homelab stops seeding a second Firecrawl
definition. Immutable client packaging and `VIBE_BACKEND_URL` remain.

This reuses native adapters and atomic writes (VI, XIII), binds authoritative
configuration to signed idempotent dispatch (XVIII, XXIII), and bounds
secret-bearing data without logging values (XVII, XXIII).
