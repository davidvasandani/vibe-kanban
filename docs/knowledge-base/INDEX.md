# Project knowledge base

Distilled, reusable knowledge from completed tasks. One topic per page; each page lists the
task ids that contributed to it. Consult this index before planning a new task; add or
update pages (and this index) when a task ships something reusable.

| Page | Summary | Contributing tasks |
| --- | --- | --- |
| [interrupted-worktree-recovery](interrupted-worktree-recovery.md) | Restart-safe WIP capture, truthful multi-repo partial-failure metadata, killed-orphan terminal state, and dirty reset/retry guards before process/history cleanup | `959a-restart-rewinds` |
| [claude-log-normalization](claude-log-normalization.md) | How `ClaudeLogProcessor` turns stream-JSON into `/entries/{i}` patches; `EntryIndexProvider` idioms; the AmpResume index-reset gotcha | `4095-thinking-tokens` |
| [collapsing-repeated-log-entries](collapsing-repeated-log-entries.md) | Server-side pattern for collapsing uninterrupted repeated log events into one entry with a `✓` per repeat | `4095-thinking-tokens` |
| [grok-executor-integration](grok-executor-integration.md) | Grok Build's ACP launch/auth and approval contract, native TOML MCP shape, and the cross-product executor integration checklist | `43bc-add-grok-to-vk` |
| [cli-tool-oauth-login](cli-tool-oauth-login.md) | Safely orchestrating durable vendor CLI login in a signed, machine-scoped PTY: eligibility gates, independent probes, fixed catalog commands, cleanup without stale-PID signalling, and socket-close handling | `5a2a-vk-cli-tool-logi` |
| [issue-status-side-effects](issue-status-side-effects.md) | Terminal-status workspace archiving: transactional and txid-covered remote updates plus level-triggered remote-to-local reconciliation using shared provider snapshots, in-flight deduplication, failure isolation, optional local context, and archive-only semantics | `2f63-auto-archive-wor`, `f464-vk-workspace-mgm` |
| [mcp-connectivity-testing](mcp-connectivity-testing.md) | Why VK is an MCP config-writer not a client; the hand-rolled JSON-RPC probe (`crates/executors/src/mcp_test.rs`) covering stdio/streamable-HTTP/legacy-SSE, the `auth_required` (401/403 + `WWW-Authenticate`) classification, the `POST /api/mcp-config/test` route, transport-normalization gotchas, and the duplex-mock test pattern | `6286-mcp-status-and-t`, `0c92-mcp-test-connect` |
| [mcp-oauth-connect](mcp-oauth-connect.md) | The Connect flow for auth-required MCP servers: discovery, DCR, PKCE, hardened outbound policy, shared-gateway storage, Cloudflare Access origin scoping, canonical connection identity, and frontend retry gotchas | `0c92-mcp-test-connect`, `205d-harden-mcp-oauth`, `4ae2-add-a-shared-mcp` |
| [shared-mcp-configuration](shared-mcp-configuration.md) | Shared MCP definitions come from native configs; the default catalog uses a canonical transport shape that adapters convert per executor, while authenticated HTTP servers can use a loopback gateway with centrally managed credentials | `a898-allow-mcp-server`, `4ae2-add-a-shared-mcp`, `c3fb-add-slack-mcp-se` |
| [remote-external-integrations](remote-external-integrations.md) | The `crates/remote` integration checklist (encrypted write-only credentials, org-admin gating, `extension_metadata` provenance, settings-section wiring), canonical format conversion at external sync boundaries, and inbound-webhook specifics: multi-tenant signature routing, ack-before-work under platform deadlines, replay idempotency via a partial unique JSONB index, Slack Block Kit limits | `fec4-vk-slack-shortcu`, `c02f-jira-sync-format` |
