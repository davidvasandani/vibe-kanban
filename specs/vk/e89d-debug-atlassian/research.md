# Evidence
- `mcp_auth.rs::exchange_and_store`: derives UUIDv5 from owner, machine, current server name, and upstream URL.
- `mcp_auth.rs::persist_gateway_assignments`: matches configured URL against upstream or generated gateway path.
- `config.rs::update_shared_mcp_servers`: explicitly supports legacy identifier migration while preserving definitions.
- `mcp_gateway/mod.rs::store_oauth_connection`: upserts by supplied UUID, preserving capability when supplied; encryption binds owner, machine, UUID, upstream URL, not mutable server name.

Therefore retaining the resolved, owner-bound UUID on reconnect updates the existing encrypted row and keeps configured paths matchable. No Atlassian-specific protocol workaround is justified by this failure.
