# Prior Knowledge: MCP Tool Count and Last-Checked Time

The project knowledge base is populated. The most relevant pages are
`docs/knowledge-base/mcp-connectivity-testing.md` and
`docs/knowledge-base/shared-mcp-configuration.md`.

## MCP connectivity testing

- Vibe Kanban normally writes agent-native MCP configuration; the explicit test
  path is its bounded, on-demand MCP client.
- `crates/executors/src/mcp_test.rs` performs `initialize`,
  `notifications/initialized`, and `tools/list`. Its exported
  `McpServerTestResult` already includes `tool_count`, so this feature does not
  need a protocol, route, schema, or generated-type change.
- Shared tests read saved native config and return one
  `SharedMcpAssignmentTestResult` per logical-server/executor assignment.
- Frontend test results are transient and are deliberately cleared after config
  reload/save so status cannot contradict the saved native configuration.
- Probe diagnostics and authentication classifications are security-sensitive;
  this feature should consume successful metadata without changing probe or
  error behavior.

## Shared MCP configuration

- The shared settings inventory is a read-oriented card surface. A card
  represents one logical server, while test results are keyed by both server
  name and executor.
- Native agent files remain the source of truth and saves may rematerialize a
  logical definition into different executor-native shapes.
- The settings dialog owns provisional state and the outer settings save/discard
  boundary owns persistence. Ephemeral health metadata must not escape or alter
  those boundaries.
- Existing cards already show transport, assignments, connection/auth state,
  and test/edit/delete actions. Tool count and checked time belong in that card
  summary rather than in configuration forms.

## Relevant implementation precedent

- `McpSettingsSection.tsx` owns the shared result map, so a frontend-only
  checked-time map can be updated beside test result ingestion and cleared at
  the same stale-state boundaries.
- Since a logical server can have multiple successful assignment results,
  aggregation must be explicit. A single equal count can be shown directly;
  differing counts should be represented as a range.
- The repository's prior MCP UI work adds settings translations to all seven
  locale files and puts pure, edge-case-heavy presentation logic in a small
  tested helper module.

## Planning constraints distilled from the knowledge base

1. Reuse `McpServerTestResult.tool_count`; do not extend the backend.
2. Keep timestamps session-local and attach them only when a response lands.
3. Key timestamps by logical server name and clear them wherever corresponding
   test results are cleared due to configuration changes.
4. Preserve existing failed/auth-required details, OAuth, save/discard, and
   transport behavior.
5. Unit-test the multi-assignment aggregation independently of the settings UI.
