# Research: Settings-Owned MCPs in Every New Session

## Existing Producer

`local-deployment` already resolves the selected coding-agent profile and reads
its native MCP map at remote dispatch. A Codex equality filter is the only
reason other supported executors do not receive `McpConfigSnapshot`.

**Decision**: Remove the executor-specific filter and reuse the existing adapter
for the dispatched agent. Do not create a second serialization path.

## Existing Consumer

The worker already validates snapshot executor identity, creates a per-execution
Codex directory, preserves Codex runtime assets with links, and writes the map
using the native adapter. The refresh path uses the same scoped file.

**Decision**: Generalize this isolation mechanism to a home overlay and store the
actual target path. Keep refresh Codex-only.

## Native Config Locations

Supported adapters target a mix of top-level (`~/.claude.json`), nested vendor
directories (`~/.gemini/settings.json`), and XDG-style paths
(`~/.config/amp/settings.json`). A flat directory copy cannot isolate every
shape while preserving login assets.

**Decision**: Construct only the target's ancestor directories, link their
non-target siblings, link all unrelated top-level home entries, and write the
target file. Reject targets outside the source home.

## Environment Boundary

Codex natively supports `CODEX_HOME`, which limits the override to Codex data.
Most other supported CLIs resolve their config from `HOME`. XDG-based agents may
honor `XDG_CONFIG_HOME` independently.

**Decision**: Preserve `CODEX_HOME` for Codex. Set `HOME` for other executors and
set `XDG_CONFIG_HOME` to the scoped `.config` when their configured path is
under `.config`. Apply values to the child process only.

## Security

The snapshot may include authorization and Cloudflare Access headers. The
existing cluster protocol bounds snapshot bytes and includes the value in
idempotency comparison without logging it.

**Decision**: Preserve the authenticated protocol and avoid messages containing
serialized definitions. Tests use synthetic non-secret values. No new secret
store or environment-variable persistence is introduced.

## Alternatives Rejected

- **Seed each worker's global vendor config**: violates Settings authority and
  introduces cross-session races.
- **Keep a repository `.mcp.json` fallback**: creates a second identifier and a
  second credential source.
- **Generate environment variables from saved headers**: changes the stored
  definition's semantics and expands secret lifetime.
- **Claim live refresh for all agents**: violates confirmed-capability rules.

## Dependencies

No new dependencies are required.

