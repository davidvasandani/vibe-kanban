# Implementation Plan: Shared HTTP Slack MCP

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical Context

- NixOS module: `homelab/modules/vibe-kanban-rebuild.nix`, instantiated on
  think2 (coordinator), think3, and think4.
- Slack MCP artifact: fork release `v1.3.0-vk.2`, launched by its existing
  verified npm wrapper with `--transport http`.
- Vibe Kanban catalog/materialization: Rust in
  `crates/executors/default_mcp.json`, `mcp_config.rs`, and
  `shared_mcp_config.rs`.
- Transport contract: Streamable HTTP at `/mcp`; native agent formats are
  produced by existing adapters.
- No database schema or new application API is required.

## Architecture & Approach

### Deployment ownership

Add a nested `slackMcp` configuration to the existing Vibe Kanban rebuild
module. It is valid only on the coordinator and owns:

- `enable`, `host`, `port`, `endpointUrl`;
- `xoxpTokenFile`, a runtime path validated to exclude the Nix store;
- exact `allowedAddresses`, defaulted explicitly by the host configuration;
- the pinned launcher URL and digest already governed by the Vibe Kanban source
  pin.

The systemd unit runs as a dynamic/private service identity, declares explicit
`HOME`, `StateDirectory`, and `CacheDirectory` paths, loads the XOXP file
through `LoadCredential`, exports it only in the service script, and executes
the launcher with `--transport http`. Nix fetches the outer launcher tarball as
a fixed-output artifact before `npx` executes it. The service binds only the coordinator LAN
address. Restart policy is local to the add-on; Vibe Kanban units neither require
nor order themselves after it.

Network policy follows the dual-firewall pattern: the NixOS firewall receives
the exact accepts when enabled, and a dedicated nftables input chain always
accepts loopback plus exact consumer IPs then drops all other traffic to the
port. Address and coordinator-role assertions fail evaluation for unsafe shapes.

### Catalog endpoint injection

Keep generic product defaults deployable by introducing
`VIBE_KANBAN_SLACK_MCP_URL`. `PRECONFIGURED_MCP_SERVERS` applies the override
only when nonblank and rewrites only the `slack` entry to canonical
`{ "type": "http", "url": ... }`. The homelab module injects the private
endpoint into coordinator Vibe Kanban and worker processes so config writes made
on either execution host use one logical definition.

Because native agent files are host-local, the worker unit atomically converges
the exact bundled Slack entry in its Claude config before launch, preserves a
backup, and fails closed on a custom same-name entry. A module assertion rejects
new non-Claude worker profiles until their native provisioning is implemented.

If product behavior requires a useful catalog outside this deployment, the
checked-in Slack entry uses a non-secret placeholder URL and metadata clearly
marks it operator-hosted; the deployment override is authoritative.

### Existing-entry migration

Extend the existing exact Slack historical-template reconciliation in
`shared_mcp_config.rs`. Recognize both:

1. the original mutable upstream stdio template; and
2. the exact shipped forked `v1.3.0-vk.2` stdio template.

Only complete shape matches migrate. The replacement is the current
preconfigured HTTP definition. Unlike stdio-to-stdio migration, no XOXP value is
preserved because the credential now belongs exclusively to the service.
Modified args, extra env keys, different commands, or additional fields remain
custom/conflicting.

### Provenance

Move the fork tag, install URL, and launcher SHA-256 contract out of assumptions
that the URL appears in the current catalog. The deployment module and Rust
tests share literals only through explicit synchronized constants/comments;
tests continue downloading the pinned launcher in the ignored audit. Add a Nix
evaluation assertion for the service command and a Rust test that detects drift
between the documented deployment pin and catalog migration fixture.

### Documentation

Update the MCP configuration guide from per-agent stdio/token setup to the
operator-hosted HTTP model. Add a homelab deployment section covering secret-file
provisioning, expected URL, systemd status, listener/firewall checks, and a
credential-safe MCP initialize/tools-list probe.

## Data Model

See `./data-model.md`. Configuration objects only; no persistent schema.

## Contracts

See `./contracts/slack-mcp-http.md` and
`./contracts/catalog-migration.md`.

## Research Notes

See `./research.md`.

## Constitution Check

- Homelab 5/31: stays inside the project-context-owned deployment module,
  declares one fleet-wide connector, exact consumers, and optional health.
- Homelab 8/21/25: private exposure is source-scoped, credentials use runtime
  files, and every option maps to a verified upstream behavior.
- Vibe Kanban XVI: the deployed service retains the immutable fork source and
  digest audit; the catalog advertises the service it actually uses.
- Vibe Kanban vendor-config principles: only exact historical templates migrate;
  custom native entries remain untouched.
- No deviation or open constitutional exception remains.

## Risks & Dependencies

- The token file must be provisioned before first service start. This is an
  explicit operator prerequisite because no authoritative 1Password reference
  was supplied.
- Generic stdio installs retain the previously documented detection-only outer
  tarball risk. This deployment closes it with a fixed-output Nix fetch before
  execution while retaining the launcher's per-platform binary verification.
- Existing saved Slack entries are distributed across native agent config files.
  Tests must cover JSON, JSONC, TOML, and Opencode shapes.
- Private source filtering assumes declared stable cluster addresses; host config
  changes must update the allowlist.
