# Clarifications: Shared HTTP Slack MCP

**Resolved during:** `/speckit.clarify` on 2026-08-06

No blocking product question remains. The open questions were resolved from the
pinned fork source, current host configuration, and the least-privilege
constitution.

## Q1: Native HTTP or bridge?

**Decision:** Use the fork's native `--transport http` mode. Tag
`v1.3.0-vk.2` accepts `stdio`, `sse`, and `http`; HTTP constructs a
Streamable HTTP server with endpoint path `/mcp`. No gateway or legacy SSE
bridge is needed.

**Rationale:** Native HTTP removes another process and dependency, preserves the
fork's own request middleware, and avoids the known inbound-auth ambiguity of
generic stdio bridges.

## Q2: Bind address, port, and client URL?

**Decision:** The coordinator hosts the singleton on
`172.16.100.102:13080`; clients use
`http://172.16.100.102:13080/mcp`. The module keeps host, port, and advertised
URL configurable.

**Rationale:** `172.16.100.102` is the already-declared coordinator address
used by think3 and think4. Port `13080` is the fork's documented default and
does not collide with the declared Vibe Kanban ports. Binding the private
interface avoids `0.0.0.0`; loopback and worker addresses are handled by the
network policy.

## Q3: Slack token provisioning?

**Decision:** The module consumes a root-readable runtime token file (default
`/var/lib/vibe-kanban-secrets/slack-xoxp-token`) through systemd
`LoadCredential=`. The module does not invent or commit a 1Password item name.
The operator may provision that file from the same XOXP token currently used by
the bundled entry, preferably from 1Password, before enabling the service.

**Rationale:** The screenshot proves a usable token exists but does not identify
an authoritative 1Password vault coordinate. Guessing a reference would create
a deploy that evaluates and then fails at runtime. A runtime-file contract is
compatible with 1Password, sops, or a one-time secure migration and keeps the
secret out of Nix, Git, process arguments, and agent configs.

## Q4: Concurrent clients and authentication?

**Decision:** Use the fork's native Streamable HTTP server for concurrent clients
without stateful bridge flags. Do not configure `SLACK_MCP_API_KEY`: a bearer
would have to be copied into every agent-readable native config. Enforce ingress
by exact private source address and bind address instead.

**Rationale:** The pinned implementation uses mcp-go's Streamable HTTP server and
its normal per-request/session handling. The upstream API key middleware applies
to HTTP tool calls, but distributing that credential contradicts the task's
central-credential goal. Exact host firewall admission plus a private bind keeps
this endpoint inside the declared cluster boundary.

## Resolved specification details

- The endpoint is optional to Vibe Kanban health and scheduling.
- Existing exact shipped stdio templates migrate; modified/custom entries do
  not.
- The existing XOXP token shown in the attachment should be rotated after the
  migration because the screenshot contains credential material; rotation is an
  operator action and outside repository changes.
