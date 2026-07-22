# Data Model: MCP Check Summary

## Existing input

`SharedMcpAssignmentTestResult`

- `server_name: string`
- `executor: BaseCodingAgent`
- `result.status: ok | failed | auth_required | unsupported`
- `result.tool_count: number | null`

## New transient state

`checkedAtByServer: Record<string, number>`

- Key: logical MCP server name.
- Value: epoch milliseconds captured when the latest response batch containing
  that server resolves.
- Lifetime: current loaded settings configuration only.

## Derived presentation model

`McpToolCountSummary`

- `minimum: number`
- `maximum: number`
- Equal minimum/maximum represents one displayed count.
- Absence represents no successful known count.

No persistent entity, migration, or serialized API field is added.
