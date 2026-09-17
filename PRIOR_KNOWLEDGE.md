# Prior knowledge — vk/e89d-debug-atlassian

Searched the existing Vibe Kanban knowledge base for OAuth, shared configuration, gateway identity, and runtime connectivity. It is populated.

- `vibe-kanban/docs/knowledge-base/mcp-oauth-connect.md`: browser callback and pasted completion share `exchange_and_store`; flows consume exchange inputs once; canonical URL identity and secret-safe errors are required.
- `vibe-kanban/docs/knowledge-base/shared-mcp-configuration.md`: gateway identity binds owner, server name, and canonical upstream URL. Reconnect preserves the local capability; assignment matching tolerates gateway port changes. Native adapters differ by executor.
- `vibe-kanban/docs/knowledge-base/cluster-mcp-runtime-connectivity.md`: gateway URLs are remapped for runtime routing; settings-owned definitions must survive dispatch. Examine authoritative configuration and URL normalization before assuming upstream credentials failed.
- `vibe-kanban/docs/knowledge-base/active-mcp-refresh.md`: configured, gateway-connected, and active agent tool state are distinct facts. A live agent may need restart after settings change.

The older shared-configuration page describes native files as authority; newer constitution and runtime notes describe settings-owned configuration. Resolve this against implementation rather than copying stale guidance.

Also searched `vibe-kanban/wiki/INDEX.md` and all wiki pages for Atlassian, OAuth, and gateway. No existing page covers shared MCP reconnect identity; the Codex credential page concerns a separate CLI authentication mechanism. A dedicated reconnect topic is appropriate.
