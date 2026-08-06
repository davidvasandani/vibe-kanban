# Cluster MCP runtime connectivity

Contributing task: `VAS-356`

An MCP configuration can be valid, persisted, and successfully tested by the
coordinator while remaining unusable by an executor on a worker. Treat these as
three separate boundaries:

1. **Persistence:** the native agent file contains the assignment.
2. **Runtime adoption:** the agent reloads and reports the server in its live
   inventory.
3. **Worker connectivity:** the server process can reach its backend from the
   node and network namespace where the executor actually runs.

## Coordinator URLs belong at the worker execution boundary

The bundled Vibe Kanban MCP normally discovers a local backend from a port file.
That is correct for a single-node deployment and wrong on a cluster worker: a
worker-local port file may be stale or describe an unrelated local process.
Workers already have an authoritative coordinator URL, so expose that same value
under the MCP's deterministic `VIBE_BACKEND_URL` override. Derive both variables
from one deployment option to prevent drift; do not copy a literal URL into an
agent catalog entry.

## Connectivity tests must run where executors run

A coordinator-side MCP test proves the coordinator's route and credentials, not
a worker's. For network-backed stdio wrappers, reproduce initialization on the
worker with the exact executable and environment materialized into the agent
config. A timeout there, paired with a successful coordinator test, is routing
evidence rather than a Codex reload defect.

## Dispatch settings-owned definitions, not deployment approximations

Deployment bootstrap can make an MCP executable available, but it cannot safely
reconstruct settings-owned headers or environment secrets. For remote Codex
executions, snapshot the selected coordinator profile's native MCP server map in
the authenticated dispatch and materialize it in an execution-scoped Codex home.
Preserve the worker's shared authentication and runtime assets via symlinks, but
never overwrite its global `config.toml`: concurrent executions may select
different definitions. Bound and validate the snapshot, bind it to the selected
executor, avoid logging its contents, and remove the scoped home when the job
ends. A worker without an existing Codex home must still be able to start when
authentication is supplied through its environment.

Do not seed any MCP through `codex mcp add` during service startup. Settings own
every definition, including `vibe_kanban`; startup mutation creates a second
authority and can silently replace a complete definition with an incomplete one.

## Widen only the required service port

When a dedicated nftables chain protects several adjacent services, split the
accept rules by consumer. Adding VK workers for Firecrawl TCP 3410 must not grant
those workers access to logmein ports 8189/8190. Keep the final targeted drop and
add a CI invariant that positively asserts the intended accepts and negatively
asserts the forbidden cross-port access.
