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
6. **Replace or merge worker MCP definitions?** Replace the selected executor's
   MCP section with the coordinator snapshot. The coordinator is authoritative;
   merging would preserve stale deployment-local definitions.
7. **What payload bound applies?** The serialized MCP snapshot is capped at 1
   MiB, well below the signed request ceiling and far above expected config size.
8. **What happens to the existing `vibe_kanban` definition?** Removing startup
   seeding is non-destructive and leaves the settings-visible native definition
   intact. Fresh deployments expose the bundled executable/template but require
   explicit settings assignment.
9. **What may Nix still own?** Executable PATH entries, service environment, and
   network connectivity. It may not mutate native MCP definitions.
10. **How broad is the invariant?** It rejects every `codex mcp add` invocation
    in the Vibe Kanban module, not only known identifiers.

## Open questions

None.
