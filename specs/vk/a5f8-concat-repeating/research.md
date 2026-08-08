# Research: MCP Identifier and Display-Label Separation

## Current failure boundary

`crates/executors/src/shared_mcp_config.rs` correctly treats server map keys as
identifiers and validates `^[a-zA-Z0-9_-]+$` before profile writes. Native reads,
merging, conflicts, tests, OAuth, and gateway identity all key on this value.
The frontend `McpServerDialog.tsx` applies an equivalent regex, but the logical
model has only `name`; it cannot retain a distinct human label.

The bundled catalog already has the right source shape:

- top-level object key: stable identifier;
- `meta.<key>.name`: display label.

`preconfiguredMcpServers()` already parses both, but only the key reaches the
draft. Existing unsafe native keys can be loaded, then cannot be confirmed by
the dialog because validation rejects the same string.

## Decision: label-only Vibe Kanban sidecar

Store `{ identifier: display_label }` in an app-owned JSON file under the
existing platform config directory (`dirs::config_dir()/vibe-kanban/`). Native
agent files remain the only source of executable MCP definitions and assignment
truth. The sidecar is presentation metadata only.

Why:

- External clients do not share a portable display-name field.
- Injecting an unknown field into definitions violates lossless codec and client
  compatibility rules.
- Current catalog derivation is insufficient for external/plugin entries and
  loses labels if catalog metadata changes.
- A database table would bind machine-local native files to a deployment DB and
  add migrations/service plumbing for a tiny host-local preference.

The sidecar is read independently. Missing, malformed, or unreadable metadata
must not prevent native MCP configuration from loading; it yields no labels plus
a scoped read error/diagnostic. Writes use temp-file + rename and owner-only Unix
permissions, matching the repository's external-config safety conventions.

Label persistence occurs only after at least one relevant native profile write
succeeds. This keeps a totally failed save from advertising a label for a server
that was never written. Partial success may persist the label because the shared
read model will contain the successfully written native server.

## Decision: one normalization contract

Keep Rust `suggested_server_identifier` authoritative and add a frontend helper
with table-driven parity fixtures because dialog validation must be immediate.
The helper follows the clarified ASCII algorithm exactly. Both suites include
the same edge cases, including punctuation runs, non-ASCII-only input, leading
separators, and `Atlassian Rovo`.

No new dependency is required.

## Decision: explicit repair of legacy unsafe keys

Native read keeps the unsafe key unchanged. Editing seeds:

- identifier: normalized suggestion;
- display label: original unsafe key (or existing sidecar label);
- original identifier: retained only for remove-plus-add tracking.

Cancel is inert. Submit + outer Save explicitly removes the original and adds
the safe identifier. Server validation continues to reject any unsafe submitted
identifier.

## Alternatives rejected

- **Relax backend validation:** rejected because Codex and other clients reject
  unsafe map keys.
- **Always silently normalize writes:** rejected because collisions, OAuth
  identity, scripts, and references make renames operationally meaningful.
- **Derive labels only from the current catalog:** rejected because it does not
  support external/plugin MCPs and is not stable over catalog changes.
- **Store labels in every native definition:** rejected because clients have
  different schemas and unknown fields may be rejected or lost.
- **Use numeric suffixes automatically:** rejected because the user should
  confirm durable external identifiers and may already have the same MCP.
