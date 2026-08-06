# Contracts

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
