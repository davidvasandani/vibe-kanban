# Project knowledge base

Distilled, reusable knowledge from completed tasks. One topic per page; each page lists the
task ids that contributed to it. Consult this index before planning a new task; add or
update pages (and this index) when a task ships something reusable.

| Page | Summary | Contributing tasks |
| --- | --- | --- |
| [claude-log-normalization](claude-log-normalization.md) | How `ClaudeLogProcessor` turns stream-JSON into `/entries/{i}` patches; `EntryIndexProvider` idioms; the AmpResume index-reset gotcha | `4095-thinking-tokens` |
| [collapsing-repeated-log-entries](collapsing-repeated-log-entries.md) | Server-side pattern for collapsing uninterrupted repeated log events into one entry with a `✓` per repeat | `4095-thinking-tokens` |
| [mcp-connectivity-testing](mcp-connectivity-testing.md) | Why VK is an MCP config-writer not a client; the hand-rolled JSON-RPC probe (`crates/executors/src/mcp_test.rs`) covering stdio/streamable-HTTP/legacy-SSE, the `POST /api/mcp-config/test` route, transport-normalization gotchas, and the duplex-mock test pattern | `6286-mcp-status-and-t` |
| [remote-external-integrations](remote-external-integrations.md) | The `crates/remote` integration checklist (encrypted write-only credentials, org-admin gating, `extension_metadata` provenance, settings-section wiring) plus inbound-webhook specifics: multi-tenant signature routing, ack-before-work under platform deadlines, replay idempotency via a partial unique JSONB index, Slack Block Kit limits | `fec4-vk-slack-shortcu` |
