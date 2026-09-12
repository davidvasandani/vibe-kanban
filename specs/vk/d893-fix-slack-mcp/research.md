# Research Notes: Fix Slack MCP Native-Configuration Conflict

## D1 - Defect layer

**Decision**: Fix the backend shared MCP canonicalization/read path.

**Evidence**:

- `docs/knowledge-base/shared-mcp-configuration.md` states that native executor
  config files are the source of truth and that `GET /api/mcp-config/shared`
  normalizes native entries, merges equivalent same-name definitions, and
  reports conflicts.
- `crates/executors/src/shared_mcp_config.rs` implements that flow:
  `load_shared_mcp_config()` calls `reconcile_snapshots()`, which groups
  same-name native entries by `normalized_fingerprint(canonical_definition())`.
- The screenshot reports a shared MCP settings conflict for one server name,
  `slack`, split as Codex versus Claude Code/Gemini/Grok. That is exactly the
  shape emitted by `reconcile_snapshots()` when fingerprints differ.

**Rejected alternative**: Fixing the frontend conflict dialog. That would hide a
backend read-model error while leaving the next save/read cycle able to produce
the same false conflict again.

## D2 - Confirmed cause

**Decision**: Reconcile the exact historical bundled Slack template
`slack-mcp-server@latest` to the current pinned fork definition while preserving
the configured token.

**Evidence**:

- Codex and Grok are configured as TOML-backed profiles with `mcp_servers`;
  Claude Code and Gemini are JSON-family profiles with `mcpServers`
  (`crates/executors/src/executors/mod.rs`).
- `canonical_definition()` already exists to collapse equivalent native shapes
  before fingerprinting. It normalizes stdio command/args/env, Opencode
  `type = local` command arrays, `env` versus `environment`, `url` versus
  `httpUrl`, and `headers` versus `http_headers`.
- Credential-safe inspection of the four native files behind the screenshot
  showed Codex already used the pinned `v1.3.0-vk.2` fork, while Claude Code,
  Gemini, and Grok still used the former bundled
  `slack-mcp-server@latest` definition. All four token hashes matched.
- Plain TOML-versus-JSON fixtures already reconciled before the fix, proving
  serialization was not the cause.

**Implementation implication**: The read path recognizes only the exact former
bundled Slack command/args/env-key shape and upgrades its install argument from
the current preconfigured catalog. Other package URLs, commands, arguments,
environment keys/values, and extra environment entries remain conflict-sensitive.

## D3 - Pinned Slack contract

**Decision**: Keep the pinned Slack release contract unchanged.

**Evidence**:

- `crates/executors/default_mcp.json` defines Slack as:
  `command: "npx"`, args `["-y", "<pinned fork release tgz>", "--transport",
  "stdio"]`, and env `SLACK_MCP_XOXP_TOKEN`.
- `crates/executors/src/mcp_config.rs` records
  `SLACK_MCP_FORK_TAG = "v1.3.0-vk.2"`,
  `SLACK_MCP_LAUNCHER_SHA256`, and `SLACK_MCP_INSTALL_SPEC`, then tests the
  catalog shape and immutable fork-artifact URL.
- `docs/integrations/mcp-server-configuration.mdx` documents the same version,
  URL shape, token env var, digest check, and operator escape hatch.
- `docs/knowledge-base/forked-mcp-server-packaging.md` explains why this fork
  must be pinned by GitHub release asset instead of using upstream `@latest`.

**Rejected alternative**: Re-pin or replace Slack as part of this task. The
repository evidence does not show a broken pin, and changing the pin would
expand the task into artifact provenance work unrelated to the false conflict.

## D4 - Credential comparison

**Decision**: Keep configured Slack credential values in the fingerprint.

**Evidence**:

- The spec requires real credential differences to remain conflicts.
- The current backend redaction exception is scoped to shared gateway bearer
  capabilities; `redact_gateway_definition()` and `redact_gateway_source()` only
  apply when the URL contains `/mcp-gateway/`.
- Slack stdio credentials are native env values, not shared gateway bearer
  capabilities.

**Testing implication**: Use fake values such as `xoxp-one` and `xoxp-two` to
prove value-sensitive conflict behavior without exposing real tokens.

## D5 - Test strategy

**Decision**: Cover both positive reconciliation and negative conflicts in
backend unit tests.

**Positive tests**:

- Codex plus Claude Code/Gemini/Grok equivalent Slack native entries reconcile
  to one server with no conflicts.
- Reconciled Slack materializes back to each native executor shape and a second
  `reconcile_snapshots()` pass remains conflict-free.

**Negative tests**:

- Different command conflicts.
- Different args conflict.
- Different transport args conflict.
- Different release artifact conflicts.
- Different env key conflicts.
- Different fake token value conflicts.

**Rejected alternative**: Testing only `default_mcp.json` or only
`canonical_definition()` pairs. That would miss the actual settings read model
where the screenshot's conflict is produced.

## D6 - Supporting artifacts

**Decision**: Do not create `data-model.md` or `contracts.md`.

**Rationale**: No persistent data model, API payload, generated type, or
frontend contract is expected to change. The relevant contract is behavioral and
already captured in `spec.md`, `clarifications.md`, and the verification plan:
equivalent native Slack definitions reconcile; semantic differences still
conflict; the pinned fork launch contract stays unchanged.

## D7 - Knowledge-base and commit completion

**Decision**: Treat the final knowledge-base update and commit as required
implementation work, not as a proposal that waits for another approval.

**Rationale**: The user explicitly requires the final knowledge-base update and
commit for this task. That instruction resolves the normal approval question
for writing reusable project knowledge.

**Implementation implication**: After validation and independent review, update
the relevant `docs/knowledge-base/` entry with any reusable implementation
knowledge learned and commit the implementation plus knowledge-base update
together. The final handoff should report the commit and validation results.
