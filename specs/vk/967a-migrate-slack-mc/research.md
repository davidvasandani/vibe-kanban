# Research: Shared HTTP Slack MCP

## Pinned fork behavior

Tag `v1.3.0-vk.2` resolves to commit
`5ef0da0c1a66615017b212817e08888d7c087c2e`.

The tagged `cmd/slack-mcp-server/main.go` accepts
`--transport http`, reads `SLACK_MCP_HOST` and `SLACK_MCP_PORT` (defaults
`127.0.0.1:13080`), and starts the server on that exact address. The tagged
`pkg/server/server.go` creates mcp-go's Streamable HTTP server with endpoint
path `/mcp`. Therefore no stdio bridge or SSE endpoint is needed.

The server accepts one of XOXP, XOXB, or XOXC+XOXD for Slack. This deployment
retains XOXP because that is the current bundled contract and provides user-level
search behavior. `SLACK_MCP_API_KEY` can protect HTTP requests, but using it
would distribute a bearer into agent config. Exact private source filtering is
the selected ingress control.

## Artifact delivery

The existing launcher tarball remains the smallest reusable delivery mechanism:
it chooses the platform binary and verifies its embedded digest before exec. The
current outer tarball SHA-256 audit remains detection-only, as documented in
`forked-mcp-server-packaging.md`. Alternatives rejected:

- build from Git source in Nix: duplicates the fork release pipeline and requires
  vendoring/fixed-output Go dependency work unrelated to transport;
- upstream npm `@latest`: loses fork attachment behavior and immutability;
- supergateway: unnecessary because the fork is already native Streamable HTTP;
- container: adds a runtime and another network boundary without benefit.

## Host selection and endpoint

Think2 is the Vibe Kanban coordinator at `172.16.100.102`. Worker coordinator
URLs and firewall lists already use that address. Think3 and think4 are
`172.16.100.103` and `172.16.100.104`. The selected URL is
`http://172.16.100.102:13080/mcp`.

## Secret provisioning

The attachment shows an existing XOXP token but not its authoritative secret
store coordinate. Embedding it, scraping it from native configs, or guessing a
1Password item are rejected. A systemd credential file is the stable deployment
contract. It can later be rendered by sops or 1Password without changing the
service.

The attachment itself contains credential material. The token should be rotated
after cutover; repository code must never quote it.

## Catalog behavior

`PRECONFIGURED_MCP_SERVERS` is a process-global lazy value and already supports
a deployment override for the bundled Vibe Kanban MCP command. A parallel Slack
URL override is consistent and narrowly scoped.

`shared_mcp_config.rs` already performs an exact Slack migration from an older
`slack-mcp-server@latest` template to the pinned fork template. Extending this
choke point is safer than writing a one-off filesystem migration because it
normalizes each executor's native representation and preserves conflicts.

