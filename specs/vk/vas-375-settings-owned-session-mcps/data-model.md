# Data Model: Settings-Owned MCP Execution Snapshot

## McpConfigSnapshot (existing wire type)

| Field | Type | Meaning |
|---|---|---|
| `executor` | string | Stable executor identifier the native map targets |
| `servers` | ordered map of JSON values | Complete settings-owned native MCP server map |

Invariants: bounded by the cluster protocol; executor must match dispatch;
contents are secret-bearing and never logged; optional for rolling compatibility.

## PreparedMcpConfig (worker-local)

| Field | Meaning |
|---|---|
| execution root | Temporary directory owned by one execution |
| scoped home | Home/config root exposed to the child |
| target config | Exact native config file written and refreshed |
| agent | Native MCP adapter/configuration |
| launch environment | Child-only overrides such as `HOME`, `CODEX_HOME`, or `XDG_CONFIG_HOME` |

Lifecycle: prepare before spawn; atomically replace the target MCP map; retain
through process lifetime and confirmed Codex refresh; remove on drop.
## Home Overlay

The overlay mirrors the source home structurally. Unrelated entries are symbolic
links. Ancestors of the target config are real directories whose non-target
children are links. The target config is a real execution-scoped file.
