# Implementation Plan: Remote MCP Refresh Rematerialization

1. Reuse VAS-356's bounded `McpConfigSnapshot` in a signed execution-scoped
   refresh request and define secret-safe worker phase outcomes.
2. Retain each remote Codex job's prepared scoped config metadata, live
   `McpRefreshHandle`, and per-execution refresh claim on `WorkerJob`.
3. Add a signed worker route that validates execution/snapshot identity,
   atomically replaces only the scoped MCP section, and calls Codex reload only
   after materialization succeeds.
4. Add the coordinator worker-client operation. Resolve the latest profile and
   settings with the dispatch resolver, find persisted execution-worker
   affinity, send the fresh snapshot, and map worker phases into the existing
   session refresh generation.
5. Preserve local refresh behavior and return unsupported for old workers,
   terminal/recovered jobs, non-Codex executors, or executions without a safe
   scoped/live control.
6. Extend safe public error categories for materialization versus Codex
   reload/bootstrap, regenerate TypeScript contracts, and retain the existing UI
   status model.
7. Test atomic preservation, A-to-B definition changes, isolation, busy claims,
   phase-safe errors, signed routing, conversation identity, and worker-side MCP
   initialize plus `tools/list`.
8. Run format, focused Rust/frontend/generated checks, independent Codex review
   to no significant findings, then update and commit reusable knowledge.

No homelab or other service change is required.
