# Research: Cluster-safe MCP runtime connectivity

## Evidence

- Codex runs with `HOME=/var/lib/vibe-kanban` and reads the expected global
  `config.toml`.
- Direct Firecrawl stdio initialization fails from think4 with a connect timeout
  to `172.16.100.101:3410`.
- think1 nftables currently allows 3410 only from think1 and think2.
- `/tmp/vibe-kanban/vibe-kanban.port` on think4 contains stale port 45993.
- The live coordinator is reachable at the configured worker URL on think2.
- `vibe-kanban-mcp` prioritizes `VIBE_BACKEND_URL` over local port discovery.

## Alternatives rejected

- **Delete the stale port file:** transient and unsafe; the next stale writer can
  recreate it, and workers should never depend on coordinator-local discovery.
- **Open 3410 to the entire LAN:** unnecessarily expands a browser-control
  surface.
- **Proxy Firecrawl through a new VK endpoint:** more plumbing than required;
  exact source-address access already matches the cluster topology.
- **Restart Codex:** does not repair either unreachable backend.
