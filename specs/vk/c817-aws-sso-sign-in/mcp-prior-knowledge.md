# Prior knowledge — vk/669e-mcp-blocks-progr

Searched Vibe Kanban wiki/INDEX.md and docs/knowledge-base/INDEX.md for MCP, transport failures, runtime connectivity, and execution lifecycle. The project knowledge base is populated.

- docs/knowledge-base/mcp-connectivity-testing.md: VK primarily writes native agent configuration. Settings connectivity probes are separate from runtime connections; failed probes do not establish failed turns.
- docs/knowledge-base/cluster-mcp-runtime-connectivity.md: persistence, runtime adoption, and worker reachability are separate boundaries. Diagnose on the execution's actual worker, preserve settings ownership, and never log credential-bearing snapshots.
- docs/knowledge-base/active-mcp-refresh.md: reload acknowledgement is not adoption; executor status and exact execution identity are authoritative. Restart is the fallback when live adoption cannot be proven.
- wiki/agent-process-lifecycle.md: protocol completion is the turn boundary; inherited output descriptors must not block terminal evidence forever. Retain true process failures and distinguish transport diagnostics from lifecycle signals.

Planning implication: correlate the reported Windows MCP line with its execution before choosing a fix. Do not assume that an rmcp worker's fatal error is fatal to the coding-agent process.
