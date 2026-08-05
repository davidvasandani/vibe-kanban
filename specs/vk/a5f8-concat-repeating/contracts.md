# Contracts: MCP Identifier and Display Label

## Shared read DTO additions

```text
SharedMcpServer {
  name: string,
  display_name: string | null,
  ...existing fields
}

SharedMcpConflict {
  name: string,
  display_name: string | null,
  ...existing fields
}
```

`name` remains the wire-compatible identifier and action target.

## Shared write DTO addition

```text
SharedMcpServerInput {
  name: string,
  display_name?: string | null,
  definition: McpServerDefinition,
  assignments: BaseCodingAgent[],
  native_overrides: Record<BaseCodingAgent, JsonValue>
}
```

Older clients omitting `display_name` remain valid through serde defaulting.

## Validation contract

- `name` must match `^[a-zA-Z0-9_-]+$`.
- identifiers must be unique in the request.
- `display_name` is trimmed; empty or identical values are stored as absent.
- label keys cannot introduce a second server identity.
- identifier and native-definition validation occurs before any write;
  label-store failures are reported separately and do not block valid native
  configuration writes.

## Dialog result

```text
{
  name: string,                  // identifier
  displayName?: string | null,  // label
  entry: JsonValue,
  assignments: BaseCodingAgent[]
}
```

For an unsafe existing name, the dialog receives both the proposed safe name
and `originalName`; the outer container uses `originalName` for explicit
remove-plus-add behavior.

## Presentation contract

- primary text: `display_name ?? name`;
- secondary identifier: shown whenever a distinct display label exists;
- React keys, test targets, auth payloads, disconnect/refresh identifiers,
  result indexes, and mutations: always `name`.
