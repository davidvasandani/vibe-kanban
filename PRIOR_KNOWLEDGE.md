# Prior Knowledge: Remote MCP Refresh

Sources reviewed: `docs/knowledge-base/active-mcp-refresh.md`,
`cluster-mcp-runtime-connectivity.md`, and `shared-mcp-configuration.md`.

- Live refresh is an executor capability. Codex reload acknowledgement means
  queued, while the next-turn paginated status snapshot is the strongest
  available adoption evidence. The protocol exposes no inventory generation
  ID, so never invent restart/reuse or last-known-good facts.
- The session-keyed coordinator already serializes generations and reports a
  second pending request as retryable busy. Browser state must reconcile from
  that backend authority rather than treating component-local state as truth.
- VAS-356 established coordinator-authoritative, settings-owned MCP snapshots
  in signed dispatch. Workers materialize them into execution-ID-scoped Codex
  homes, share authentication/runtime assets through symlinks, leave global
  `config.toml` untouched, validate size/executor identity, and remove the home
  at job end.
- Worker-side testing is required because coordinator persistence/connectivity,
  live agent adoption, and worker network connectivity are separate boundaries.
- Shared settings ultimately derive from native executor files; the existing
  profile resolver and native-shape adapter are the one authoritative read/write
  convention. Operational identity is the stable native server identifier.
- Atomic agent-config helpers preserve unrelated vendor configuration. Errors
  and public status must never include definitions, environment values,
  authenticated URLs, tokens, or raw subprocess output.

Implication: refresh must reuse the dispatch resolver, route via persisted
execution-worker affinity, edit the already-live scoped config in place, retain
the worker's Codex control instead of probing independently, and preserve the
existing pending-to-confirmed lifecycle.
