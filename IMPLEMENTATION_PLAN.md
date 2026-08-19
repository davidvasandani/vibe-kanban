# IMPLEMENTATION_PLAN — Add Brink MCP to VK (`vk/5f7c-add-brink-mcp-to`)

Small, additive change: one catalog entry + docs. Derived from `SPEC.md` and
`PRIOR_KNOWLEDGE.md`.

## Files to change

1. `crates/executors/default_mcp.json`
2. `docs/integrations/mcp-server-configuration.mdx`

No Rust code, no generated types, no frontend code, no new assets (icon omitted).

## Step 1 — Add the `brink` launch entry

Insert after the `gmail` server block, before `meta`:

```json
"brink": {
  "command": "npx",
  "args": ["-y", "brink-pos-mcp-server"],
  "env": {
    "BRINK_ACCESS_TOKEN": "YOUR_TOKEN",
    "BRINK_LOCATION_TOKEN": "YOUR_TOKEN"
  }
},
```

Rationale (see SPEC "Design decision"): the package ships a prebuilt `dist/`
with a `bin` and no `prepare` script → plain unpinned npm launcher, like `exa`.
`BRINK_API_URL` is optional and omitted (defaults to Brink production).

## Step 2 — Add the `brink` meta entry

Insert after the `gmail` meta block:

```json
"brink": {
  "name": "Brink POS",
  "description": "Send test orders and check OLO connectivity on Brink POS registers",
  "url": "https://github.com/davidvasandani/mcp-brink"
}
```

No `icon` (matches Slack/Gmail); UI falls back to no-icon rendering.

## Step 3 — Document the connector

Add a `### Brink POS connector` subsection to
`docs/integrations/mcp-server-configuration.mdx` after the Gmail section:
- what it is (stdio SOAP client for Brink POS test ordering / OLO health),
- the launch entry,
- env vars: `BRINK_ACCESS_TOKEN` (required), `BRINK_LOCATION_TOKEN`,
  `BRINK_API_URL` (optional),
- the LocationToken `==` Base64-padding gotcha,
- note the plain-npx form assumes `brink-pos-mcp-server` is resolvable by `npx`
  on the agent host (published to the npm registry the deployment uses).

## Step 4 — Verify

- `default_mcp.json` is valid JSON (`node -e "JSON.parse(...)"` or a parse
  check) and diff-clean apart from the two additions.
- `cargo test -p executors` (catalog parses via `PRECONFIGURED_MCP_SERVERS`;
  no test regresses).
- `pnpm run check` (frontend types/lint) — `preconfiguredMcpServers()` yields a
  `brink` item.
- `pnpm run format` before finishing.

## Step 5 — Review, knowledge, PR (pipeline stages 11–13)

- Codex review of the diff; address confirmed findings.
- Record the reusable "how to add a bundled MCP server to VK" knowledge in the
  KB, tagged `vk/5f7c-add-brink-mcp-to`; refresh the index; commit.
- Open and merge the PR against the base branch.

## Risks / notes

- **External prerequisite:** `brink-pos-mcp-server` must be `npx`-resolvable on
  agent hosts. If the user distributes it as a private fork instead, switch to
  the Slack-style pinned release tarball + pin machinery (does not change the UI
  wiring). Flagged as the spec's key open question for `/speckit.clarify`.
- No catalog test enumerates all servers, so the addition cannot break existing
  Rust/TS tests; risk is limited to JSON validity and doc accuracy.
