# Prior Knowledge: Settings-Owned MCP Definitions

Relevant project knowledge exists in two pages:

- `shared-mcp-configuration.md` correctly establishes that catalog presence is
  distinct from native agent materialization, but its older recommendation to
  seed deployment-owned definitions conflicts with the now-available
  authenticated dispatch snapshot mechanism.
- `cluster-mcp-runtime-connectivity.md` records the newer rule: deployment
  startup must not create a second authority for a settings-managed definition;
  workers receive exact definitions in execution-scoped Codex homes.

Distilled guidance for this change:

1. Settings should be the single authority for every MCP definition.
2. Nix owns immutable executable availability and runtime environment only.
3. Service startup must not mutate native MCP tables.
4. Remote execution uses the bounded, authenticated snapshot already shipped in
   VAS-356.
5. Update the older knowledge page so future work does not reintroduce startup
   seeding.
