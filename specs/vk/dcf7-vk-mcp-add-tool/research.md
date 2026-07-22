# Research: MCP Tool Count and Last-Checked Time

## Existing tool-count source

`crates/executors/src/mcp_test.rs` already runs `tools/list` and returns
`tool_count` on successful results. The shared test endpoint nests this result
inside `SharedMcpAssignmentTestResult`. Decision: reuse it unchanged.

Rejected alternative: add a timestamp to the backend DTO. That time would
describe backend probe completion rather than when the UI received the result,
would require generated type churn, and still would not provide persistence.

## Timestamp ownership

The current settings component owns ephemeral results and clears them after
load/save. Decision: store per-server epoch milliseconds beside those results
and mirror their invalidation lifecycle. Capture the time after the response
promise resolves so “checked” never implies an in-flight attempt completed.

Rejected alternative: stamp at request start. A slow or timed-out check would
appear fresher than it actually is.

## Multi-executor counts

A logical shared server can be tested through several executor-native entries.
Counts can legitimately differ because native configuration or server behavior
can vary. Decision: include only `status === 'ok'` values with a numeric
`tool_count`; show one deduplicated count or a min/max range.

Rejected alternatives: first-result wins (order-dependent), sum (double-counts
the same tools), or per-executor labels (too much card detail for the request).

## Formatting and localization

The settings screen already uses react-i18next; browser `Intl.DateTimeFormat`
is available without dependencies. Decision: helper returns data, translations
own labels/pluralization, and `Intl` formats the time with active i18n language.

## Testing surface

There is no dedicated rendered-DOM test for `McpSettingsSection`; its async
machine/project context makes a new harness disproportionate for this small
display change. Decision: extract deterministic aggregation/formatting logic to
a pure helper and test it exhaustively, then validate integration with TypeScript
and existing MCP tests.
