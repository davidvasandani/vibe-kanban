# Contract: Slack MCP Streamable HTTP

## Endpoint

`POST http://172.16.100.102:13080/mcp`

The endpoint is private to loopback and declared Vibe Kanban cluster source
addresses. It is plain HTTP on the trusted cluster LAN and is not publicly
routed.

## Protocol

1. Client sends JSON-RPC `initialize` with an MCP protocol version.
2. Client retains and forwards `Mcp-Session-Id` if the server returns one.
3. Client sends `notifications/initialized`.
4. Client sends `tools/list` and may call returned tools.
5. Client accepts JSON or SSE-framed Streamable HTTP responses according to MCP.

## Authentication and secrets

- Slack upstream authentication: `SLACK_MCP_XOXP_TOKEN` exists only inside the
  supervised server process.
- Client authentication: exact host source admission; no bearer header is
  written to agent config.
- Unauthorized network sources receive no application access because the host
  firewall drops the TCP flow.

## Failure contract

Connection refused, timeout, non-2xx, invalid MCP response, and Slack provider
startup failure are Slack-integration failures only. They do not alter Vibe
Kanban health, worker leases, scheduling, or task ownership.

