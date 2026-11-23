# Prior knowledge — 9784-auto-auth-renew

Searched wiki/ and docs/knowledge-base/ for gateway, reauth, and wakeup.

- wiki/unattended-reauth.md: reuse server-defined target registry, detached bounded runs, independent verification, sweep backoff, and shared Entra refusal gate. Network failure is unknown, not expired. Persistent browser profile requires serialized ownership; credentials may only go to trusted Entra HTTPS origins.
- wiki/mcp-oauth-connection-identity.md: assigned gateway UUID and local bearer must survive reconnects/name normalization. Re-deriving identity leaves the old row serving expired credentials. Preserve owner/machine binding and reject removed assignments.
- docs/knowledge-base/mcp-oauth-connect.md: executors/mcp_oauth owns OAuth metadata validation, PKCE, registration and exchange. Gateway stores encrypted upstream credentials while agents hold local capabilities. Existing refresh-on-401 exists; expiry persistence and auto-auth integration are missing.
- wiki/durable-session-wakeups.md: wake-ups are durable session-scoped deliveries; cancellation must retain scope validation. Investigate nested route parameter extraction rather than weakening scope requirements.

These findings constrain the implementation to extend existing ownership, OAuth, and auth registry machinery; no upstream sgsc or Cloudflare service change is needed.
