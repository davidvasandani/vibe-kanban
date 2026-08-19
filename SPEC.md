# SPEC: Add Brink POS MCP server to the Vibe Kanban catalog

**Task:** `vk/5f7c-add-brink-mcp-to` — "Add Brink MCP to Vk"

## Summary

Add the **Brink POS** MCP server to Vibe Kanban's bundled "Popular MCP Servers"
catalog so users can add it to a coding agent from the MCP settings UI with one
click, then fill in their own Brink credentials. The Brink server is a stdio
MCP server (`brink-pos-mcp-server`) that talks to Brink POS over its SOAP API to
place test orders, read order details/status, test OLO (online-ordering)
connectivity, and read a register's current business date.

The catalog lives in `crates/executors/default_mcp.json`. Adding a server means
adding (1) a launch entry keyed by a protocol-safe identifier and (2) a matching
`meta` entry (display name, description, docs URL) that the settings UI reads.

## Background / what already exists

- `crates/executors/default_mcp.json` holds every preconfigured server plus a
  `meta` block. `PRECONFIGURED_MCP_SERVERS` in
  `crates/executors/src/mcp_config.rs` parses this file at startup and applies
  per-agent adapters; nothing enumerates the catalog exhaustively, so a new key
  is additive and breaks no test.
- The frontend helper `preconfiguredMcpServers()`
  (`packages/web-core/src/shared/lib/sharedMcpSettingsState.ts`) lists every
  top-level key except `meta`, reading `name`/`description`/`icon` from
  `meta[key]` (all optional; `icon` may be omitted, as Slack and Gmail do).
- Existing catalog precedents:
  - Plain published npm packages launched unpinned via `npx -y <pkg>`
    (`exa`, `dev_manager`, `playwright`, `chrome_devtools`, `vibe_kanban`).
  - `slack` — a **pinned GitHub release tarball** with coordinated Rust
    constants + a Renovate custom manager + a scheduled digest audit.
  - `gmail` — a **pinned `github:` git spec** (fork builds itself via a
    `prepare` script), hand-bumped.

## The Brink server (from the provided source)

- Package name: `brink-pos-mcp-server` (`package.json`), `type: module`,
  `bin: dist/index.js`, `files: ["dist"]`. Prebuilt `dist/` ships in the
  package; there is **no** `prepare` script and `dist/` is `.gitignore`d.
- Transport: stdio (MCP SDK default) — no transport flag needed.
- Environment (confirmed in `src/index.ts` / `src/services.ts`):
  - `BRINK_ACCESS_TOKEN` — **required** (`process.env.BRINK_ACCESS_TOKEN`).
  - `BRINK_LOCATION_TOKEN` — default LocationToken; per-request override is also
    supported. LocationTokens are Base64 and must keep their `==` padding.
  - `BRINK_API_URL` — **optional**, defaults to
    `https://api13.brinkpos.net/Ordering.svc`.
- Tools: `brink_send_order`, `brink_get_order`, `brink_get_order_status`,
  `brink_test_olo`, `brink_get_current_business_date` (plus additional
  order-listing tools in newer source).

## Functional requirements

- FR-1: `brink` appears in the bundled catalog and renders in the MCP settings
  "Popular MCP Servers" list with a human name, a description, and a docs URL.
- FR-2: Adding it writes a stdio launch entry an agent can run unmodified once
  the user supplies real credentials: `npx -y brink-pos-mcp-server` with
  `BRINK_ACCESS_TOKEN` and `BRINK_LOCATION_TOKEN` present as editable
  placeholders (`YOUR_TOKEN`), mirroring how every other catalog entry ships
  secret placeholders.
- FR-3: The server identifier is `brink` — matches
  `^[a-zA-Z0-9_-]+$` (`is_valid_server_identifier`) and supports the `_2`, `_3`
  duplication suffixing used for multiple instances.
- FR-4: `default_mcp.json` stays valid JSON; every existing server and its
  `meta` entry are unchanged.
- FR-5: Documentation in `docs/integrations/mcp-server-configuration.mdx`
  describes the Brink connector, its env vars, and the LocationToken `==`
  padding gotcha.

## Design decision: distribution / launch reference

The Brink package is built to be installed and run as `npx brink-pos-mcp-server`
(it publishes a prebuilt `dist/` and exposes a `bin`). The catalog entry
therefore uses the **plain unpinned npm launcher** form
(`npx -y brink-pos-mcp-server`), consistent with the majority of catalog entries
(`exa`, `dev_manager`, …).

Rejected alternatives and why:
- **`github:` git spec (Gmail-style)** — a `github:` install clones source and
  runs `prepare` to build. Brink has no `prepare` script and `.gitignore`s
  `dist/`, so this form would install unbuilt source and fail to launch.
- **Pinned GitHub release tarball (Slack-style)** — works with a prebuilt
  `dist/`, but requires a published release to *pin and verify* (constitution
  principle 24: pin only what is proven to exist) plus the coordinated Rust
  constant / Renovate manager / digest-audit machinery. That is disproportionate
  to "add the catalog entry" and is only warranted if a fork must diverge from a
  published package (Slack's reason). It remains the fallback if Brink is
  distributed as a private fork instead of a published package.

**External prerequisite (out of VK-repo scope):** `brink-pos-mcp-server` must be
resolvable by `npx` on agent hosts — i.e. published to the npm registry the
deployment uses (public npm or a configured private registry). If the user
prefers a pinned private fork instead, switch the entry to the Slack-style
release-tarball form and add the corresponding pin machinery. This choice does
not change the UI wiring.

`icon` is omitted (as with Slack and Gmail), so no asset is added.

## Out of scope

- Publishing `brink-pos-mcp-server` to any registry.
- Homelab deployment wiring / secret injection (credentials are entered per
  server in the UI via the `YOUR_TOKEN` placeholders, like every other entry).
- Any change to the Brink server source itself.
- Adding Renovate/pin/digest-audit machinery (only needed for the rejected
  tarball form).

## Acceptance criteria

- [ ] `crates/executors/default_mcp.json` parses as valid JSON and contains a
      `brink` server entry and a `meta.brink` entry.
- [ ] `preconfiguredMcpServers()` returns an item with `key: "brink"`, a
      non-empty `name` and `description`.
- [ ] `cargo test -p executors` and `pnpm run check` pass (no catalog test
      regresses).
- [ ] `docs/integrations/mcp-server-configuration.mdx` documents Brink.
- [ ] Codex review of the diff reports no significant findings.
