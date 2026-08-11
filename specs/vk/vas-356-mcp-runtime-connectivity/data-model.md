# Data model

No persistent application data changes.

Configuration relationships:

- `worker.coordinatorUrl` → worker unit `VK_WORKER_COORDINATOR_URL`
- `worker.coordinatorUrl` → worker unit `VIBE_BACKEND_URL`
- VK worker hosts → source IPv4 addresses `{172.16.100.103, 172.16.100.104}`
- Firecrawl service → think1 TCP port `3410`

`McpConfigSnapshot` is ephemeral protocol data containing
`executor: BaseCodingAgent` and `servers: HashMap<String, Value>`. It is not
stored in SQLite, exposed by a new API, or logged. `ExecutionDispatch` gains an
optional snapshot; absence retains older local-resolution behavior.
