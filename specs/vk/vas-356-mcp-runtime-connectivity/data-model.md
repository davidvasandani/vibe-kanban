# Data model

No persistent application data changes.

Configuration relationships:

- `worker.coordinatorUrl` → worker unit `VK_WORKER_COORDINATOR_URL`
- `worker.coordinatorUrl` → worker unit `VIBE_BACKEND_URL`
- VK worker hosts → source IPv4 addresses `{172.16.100.103, 172.16.100.104}`
- Firecrawl service → think1 TCP port `3410`
