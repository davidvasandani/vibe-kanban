# Clarifications: Cluster-safe MCP runtime connectivity

## Resolved

1. **May the Firecrawl service policy be changed?** Yes. The user explicitly
   authorized a Nix change in the homelab repository.
2. **Which sources are VK workers?** The current deployed worker-role hosts are
   think3 (`172.16.100.103`) and think4 (`172.16.100.104`). Think2 remains the
   coordinator/Cloudflare connector and think1 remains the service host.
3. **Should every protected port be widened?** No. Only Firecrawl TCP 3410 gains
   worker access. TCP 8189 and 8190 retain think1/think2-only access.
4. **Which backend URL should workers pass to the MCP?** The existing
   `worker.coordinatorUrl` option, exposed as both `VK_WORKER_COORDINATOR_URL`
   and `VIBE_BACKEND_URL`.
5. **Is a Codex version change needed?** No. Direct initialization established
   network/address failures, not a reload-protocol or version failure.

## Open questions

None.
