# Implementation Plan: Remote MCP Configuration Synchronization

1. Extend `ExecutionDispatch` with an optional, size-bounded MCP snapshot that
   identifies the executor and contains only its canonical MCP server map.
2. At coordinator dispatch construction, resolve the selected executor profile,
   read its native config through the existing adapter, and attach its MCP server
   section without logging values.
3. Include the snapshot in the stable request digest so an idempotent replay
   cannot silently change credentials or definitions.
4. On the worker, validate snapshot executor identity and serialized size before
   any agent process starts.
5. Materialize the snapshot with the existing native config reader/writer and
   adapter, updating only the MCP section and preserving unrelated settings.
6. Add protocol, coordinator, worker, preservation, mismatch, oversize, and
   backward-compatibility tests.
7. Update generated types only if a public API type changes; format and run the
   focused Rust test suites.
8. Remove the homelab deployment-owned Firecrawl seed after synchronized MCP
   settings are proven, while retaining immutable client packaging and the
   Vibe backend URL worker environment.
9. Run the independent Codex review loop, fix confirmed findings, then update
   project knowledge and merge the Vibe and homelab PRs.
