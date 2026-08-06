# Feature Specification: Cluster-safe MCP runtime connectivity

**Task:** VAS-356
**Scope:** Vibe Kanban worker runtime and explicitly authorized Firecrawl ingress

## User problem

An MCP can be saved, tested successfully by the coordinator, and still be
unusable by Codex because Codex runs on a cluster worker. `vibe_kanban` selects a
stale worker-local backend port and Firecrawl rejects worker source addresses.

## Functional requirements

- **FR-1:** Worker-launched processes receive the configured coordinator URL in
  the environment variable consumed by the bundled Vibe Kanban MCP.
- **FR-2:** The worker's existing coordinator variable remains unchanged and
  both values are derived from one Nix option.
- **FR-3:** Firecrawl port 3410 accepts the coordinator and every configured VK
  worker address.
- **FR-4:** Logmein ports 8189 and 8190 retain their current source allowlist.
- **FR-5:** Other source addresses remain denied for all three protected ports.
- **FR-6:** Focused evaluation prevents future drift in environment propagation
  and source-address scoping.

## Success criteria

- Evaluated worker service environments contain identical non-empty coordinator
  URLs under both variable names.
- Rendered think1 nftables rules widen only TCP 3410 to VK workers.
- Nix evaluation and repository checks pass.
- Following deployment, direct Firecrawl MCP initialization and a Vibe Kanban
  MCP API call succeed from a VK worker.

## Non-goals

- Changing Codex's pinned version or reload protocol.
- Making Firecrawl generally LAN-accessible.
- Reworking MCP catalog persistence or the Settings UI.
