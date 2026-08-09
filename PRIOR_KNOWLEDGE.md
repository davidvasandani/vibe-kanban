# Prior Knowledge: Legacy MCP Identifier Migration

Relevant knowledge exists in `shared-mcp-configuration.md` and
`multi-instance-catalog-templates.md`.

- Native map keys are protocol identifiers and must match
  `^[a-zA-Z0-9_-]+$`; display labels belong in `mcp-display-labels.json`.
- The shared normalizer already maps `Atlassian Rovo` to `atlassian_rovo`.
- Ordinary reads must not silently rename native keys because collisions and
  multi-profile disagreement require an explicit, transactional decision.
- Operational state—assignments, tests, refresh, and authentication—is keyed by
  the stable identifier.
- Collision checks must include configured servers and unresolved conflicts.

The implementation should therefore expose a deterministic migration during the
save/reconciliation boundary, preserve the legacy label in metadata, and refuse
ambiguous migrations without partial native writes.
