# Data Model: Shared HTTP Slack MCP

No database entities are added.

## Deployment configuration

`slackMcp`:

- `enable: bool`
- `host: IPv4 string` — exact private bind address
- `port: 1..65535 integer`
- `endpointUrl: absolute http URL` — must end in `/mcp`
- `xoxpTokenFile: absolute non-store runtime path`
- `allowedAddresses: list<IPv4>` — exact consumer addresses, no CIDRs
- `enabledTools: list<string>` — optional exact upstream capability list

Invariants:

- enabled only for `clusterRole = coordinator`;
- host and every allowed address are valid IPv4 literals;
- token path is absolute and outside `/nix/store`;
- endpoint URL host/port agree with the bind contract unless deliberately
  overridden with an assertion-backed documented reason;
- loopback is implicit firewall admission and is not stored as a consumer.

## Catalog definition

Canonical current Slack definition:

```json
{
  "type": "http",
  "url": "http://172.16.100.102:13080/mcp"
}
```

The checked-in value is overridden by `VIBE_KANBAN_SLACK_MCP_URL` in the
homelab deployment. It contains no headers, token, command, args, or env.

## Historical definition classifier

Inputs:

- server name;
- canonical transport;
- complete command;
- ordered argument vector;
- complete environment key/value shape;
- absence of extra fields.

Output:

- `ExactHistoricalBundled` -> replace with current HTTP definition;
- `Current` -> preserve;
- `Custom` -> preserve and participate in normal conflict reporting.

The classifier does not persist state and never copies the historical XOXP value
into the HTTP replacement.

