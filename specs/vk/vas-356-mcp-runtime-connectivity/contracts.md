# Contracts

## Deployment ownership contract

- Vibe Kanban settings exclusively own MCP server identifiers and definitions.
- Nix may supply commands, environment, and routes but performs no native MCP
  table mutation.
- The focused homelab check rejects `codex mcp add` anywhere in the Vibe Kanban
  rebuild module.

## Worker environment contract

For `clusterRole = "worker"`, the evaluated service environment contains:

```text
VK_WORKER_COORDINATOR_URL=<worker.coordinatorUrl>
VIBE_BACKEND_URL=<worker.coordinatorUrl>
```

## Firecrawl ingress contract

```text
172.16.100.101, 172.16.100.102 → TCP 8189, 8190, 3410: accept
172.16.100.103, 172.16.100.104 → TCP 3410: accept
all other sources                     → TCP 8189, 8190, 3410: drop
```

## Remote MCP snapshot contract

`ExecutionDispatch.mcp_config_snapshot` contains an executor identity and its
native-shape MCP server map. It is optional for rolling compatibility. When
present, identity must match the dispatched action, canonical serialized size
must not exceed 1 MiB, content participates in `request_digest`, and the worker
replaces only that executor's MCP section before spawn. Errors never render
definition values.
