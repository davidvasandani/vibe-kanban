# MCP OAuth connection identity

Shared gateway connection UUIDs are durable identities. The first connection
uses UUIDv5 derived from owner/machine, configuration name, and canonical
upstream URL. That formula is only a creation rule: configuration identifiers
can later change while native definitions retain their gateway URLs.

In particular, normalizing `Atlassian Rovo` to `atlassian_rovo` preserves the
old `/mcp-gateway/{id}` assignment. Re-deriving the UUID during reconnect stores
credentials in a different row and then fails assignment matching with
“No matching MCP assignments remained after OAuth completed”. The original
row continues serving old credentials and may still appear connected.

Resolve the configured gateway UUID with the current owner/machine binding at
OAuth start, retain it in process-local flow state, and carry it through both
browser and pasted callback completion. Reuse that UUID and the existing local
capability when upserting encrypted credentials. Reject missing/inaccessible
bound connections rather than treating the gateway itself as an OAuth upstream.

Assignment matching tolerates local gateway port changes, but still requires
the current named entry to target the retained gateway path or original
canonical upstream. Deleted or replaced assignments must not be recreated.
A saved connected state is not a live connectivity guarantee, and existing
agent processes may need restart after configuration changes.

Regression coverage in `crates/server/src/routes/mcp_auth.rs` reproduces the
name-derived UUID mismatch across Claude, Codex, and Gemini native shapes,
checks initial connection compatibility and unrelated assignment exclusion.
A live OAuth browser grant remains distinct from automated regression evidence.

## Contributed by

- `vk/e89d-debug-atlassian`
