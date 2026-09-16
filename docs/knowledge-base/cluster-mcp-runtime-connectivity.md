# Cluster MCP runtime connectivity

Contributing tasks: `VAS-356`, `VAS-375`, `268b-debug-vk-error`,
`vk/f558-update-vk-to-aut`, `vk/1d23-vk-worker-output`

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

The same rule applies to settings-owned shared gateway URLs. The coordinator
persists a loopback `/mcp-gateway/<connection-id>` URL because that is correct
for local executors. During remote materialization, the worker replaces only a
parsed loopback gateway authority with its configured coordinator authority;
direct MCP URLs and every non-URL field remain unchanged. The coordinator
gateway remains fail-closed to arbitrary remote peers: deployment supplies the
workers' explicit outbound source IP addresses, the server admits only those
addresses plus loopback, and the unguessable per-connection bearer is still
required. Direct-LAN deployments may derive literal worker hosts as the default;
NAT, proxy, load-balancer, and hostname deployments must declare the observed
source addresses separately from dispatch destinations.

## Scoped agent homes must satisfy vendor path trust

Configuration isolation does not require a general temporary directory. Codex
refuses to create PATH helper aliases when `CODEX_HOME` is beneath `/tmp`, so a
remote execution's scoped home lives under the worker's private state directory
(`worker.stateDir/mcp-config/<execution-id>/codex` in the Nix deployment). The
execution UUID still isolates concurrent configs, the prepared-config owner
still removes that exact child at teardown, and the global Codex home remains
the source of linked authentication/runtime assets rather than a write target.
Because a crash bypasses teardown and recovery interrupts rather than resumes
old jobs, worker startup clears and recreates the dedicated `mcp-config` root
with owner-only permissions before accepting executions.

For Codex, the worker also writes one exact project trust entry into the scoped
`config.toml` before launch. The key is the canonical action directory after any
initial, follow-up, or review offset has been authorised; do not trust a broader
workspace or inferred Git root. Keep the original workspace directory separately
for repository discovery and commit reminders.

Use native TOML values while adding trust. Preserve unrelated settings, other
project entries, and fields beside the selected project's `trust_level`. An
absent MCP snapshot preserves source MCP definitions, while a supplied empty
snapshot clears them. Refresh must read the existing scoped file and replace
only MCP settings so it cannot erase trust or recreate state after terminal
cleanup.

The scoped home remains disposable, but conversation data does not. Create the
persistent source `sessions` directory before building the overlay, then link it
into the scoped home. This ensures a first execution on a fresh worker does not
lose its rollout when the execution-owned directory is removed.

<Note>
Writing and parsing the scoped trust entry proves materialisation. Only a launch
with the deployed Codex executable proves that Codex adopted project-local
configuration, hooks, and execution policies.
</Note>

Treat repository skill errors separately from scoped-home routing. A skill must
start and end YAML frontmatter with `---`; commit `efe4dd7e` fixed the historical
`n8n-browser-automation` copies. An error naming an older workspace that lacks
that commit requires updating or recreating that workspace, not weakening the
skill parser or silently rewriting unrelated worktrees.

## Connectivity tests must run where executors run

A coordinator-side MCP test proves the coordinator's route and credentials, not
a worker's. For network-backed stdio wrappers, reproduce initialization on the
worker with the exact executable and environment materialized into the agent
config. A timeout there, paired with a successful coordinator test, is routing
evidence rather than a Codex reload defect.

## Dispatch settings-owned definitions, not deployment approximations

Deployment bootstrap can make an MCP executable available, but it cannot safely
reconstruct settings-owned headers or environment secrets. For every MCP-capable
remote executor, snapshot the selected coordinator profile's native MCP server
map in the authenticated dispatch and materialize it in an execution-scoped
native config. Preserve the worker's shared authentication and runtime assets via
symlinks, but never overwrite its global vendor config: concurrent executions may
select different definitions. Bound and validate the snapshot, bind it to the
selected executor, avoid logging its contents, and remove the scoped tree when
the job ends.

Use the executor's real configuration boundary. Codex supports a narrow
`CODEX_HOME`; most other agents require an execution-scoped `HOME`. If an agent
uses a custom `XDG_CONFIG_HOME` outside `$HOME`, scope and redirect that XDG root
instead of rejecting it or assuming `~/.config`. Build real directories only
along the native config path and link their non-target siblings so login/session
assets remain available without letting native config writes reach the worker's
global file or its backup.

Do not seed any MCP through `codex mcp add` during service startup. Settings own
every definition, including `vibe_kanban`; startup mutation creates a second
authority and can silently replace a complete definition with an incomplete one.
Repository `.mcp.json` entries are also a competing authority when they define
the same service under another identifier or expect environment placeholders
that Settings never creates. Remove those duplicates as part of the coordinated
Settings-owned migration; keep deployment ownership limited to executables,
routes, and runtime prerequisites.

## Widen only the required service port

When a dedicated nftables chain protects several adjacent services, split the
accept rules by consumer. Adding VK workers for Firecrawl TCP 3410 must not grant
those workers access to logmein ports 8189/8190. Keep the final targeted drop and
add a CI invariant that positively asserts the intended accepts and negatively
asserts the forbidden cross-port access.

## Keep Codex SQLite indices out of disposable path aliases

Contributed by: `vk/1d23-vk-worker-output`.

Linking persistent `sessions` into an execution-scoped Codex home preserves the
files, but not the absolute alias paths recorded in a shared Codex state database.
Once `PreparedMcpConfig` removes that execution root, indexed paths through it
become stale. For a reported thread, an on-host check found the old alias absent
and the corresponding persistent rollout still present. Codex's rollout lookup
logs `state db returned stale rollout path` and falls back to file discovery.

Initial scoped configuration sets `sqlite_home` to a private `sqlite` child of
that execution home, overriding source configuration and the vendor environment
fallback. The worker reserves a real owner-only directory there: if its generic
overlay linked a same-named source entry, it removes only that alias before
creating the private directory. Database files and WAL/SHM sidecars then share one
execution lifetime. MCP refresh preserves `sqlite_home`; persistent sessions and
authentication remain linked, and source configuration/databases remain untouched.

The native key and precedence were verified in the pinned Codex `rust-v0.144.1`
source (`44918ea`, `core/config.schema.json` and `core/src/config/mod.rs`). Rollout
fallback behavior is in `rollout/src/list.rs`. Fresh indices can incur backfill
cost. This change applies to new scoped executions and deliberately does not
rewrite existing vendor databases or claim that every stale path means a deleted
transcript. Preserve the error diagnostic rather than merely hiding it.
