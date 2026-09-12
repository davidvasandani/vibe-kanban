# Technical Plan: Fix Slack MCP Native-Configuration Conflict

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context

- **Backend surface**: shared MCP read/write logic lives in
  `crates/executors/src/shared_mcp_config.rs`. `load_shared_mcp_config()` reads
  native agent files, `reconcile_snapshots()` groups same-name servers by
  `normalized_fingerprint()`, and `materialize_definition()` writes the shared
  definition back into each executor's native shape.
- **Native config source**: `CodingAgent::get_mcp_config()` in
  `crates/executors/src/executors/mod.rs` maps Codex and Grok to TOML
  `mcp_servers`, and Claude Code/Gemini to JSON-family `mcpServers`.
- **Bundled Slack contract**: `crates/executors/default_mcp.json`,
  `crates/executors/src/mcp_config.rs`, and
  `docs/integrations/mcp-server-configuration.mdx` agree on the pinned
  `davidvasandani/slack-mcp-server` `v1.3.0-vk.2` release asset. This task must
  not change the command, args, env variable, release tag, digest, metadata URL,
  or docs unless investigation proves that pin is wrong.
- **Existing guardrails**: `mcp_config.rs` already has Slack shape and digest
  tests. Those tests are part of the security contract for the forked launcher
  and must remain intact.
- **Confirmed cause**: credential-safe inspection showed Codex had the current
  pinned fork definition while Claude Code, Gemini, and Grok retained the
  former bundled `slack-mcp-server@latest` definition. Plain TOML-versus-JSON
  fixtures already reconciled. The fix is therefore a narrow migration for that
  exact historical bundled template in the shared read path.

## Architecture & Approach

### 1. Add a focused failing regression

Add tests in `crates/executors/src/shared_mcp_config.rs` that construct
`NativeProfileSnapshot` values resembling persisted native configs:

- Codex: TOML-parsed `mcp_servers.slack` value with `command`, `args`, and
  `env`.
- Grok: TOML-parsed `mcp_servers.slack` value with the same logical command,
  args, and env.
- Claude Code and Gemini: JSON-family `mcpServers.slack` values with the same
  logical command, args, and env.

Drive them through `reconcile_snapshots()`, not a frontend helper. The first
test should reproduce the screenshot's input shape and assert:

- `response.conflicts` is empty,
- exactly one `slack` server is returned,
- the server has four assignments,
- `source_kind` is `Reconciled`,
- every native source fingerprint is present and equal.

Before fixing code, confirm the current behavior fails or, if it already passes,
adjust the fixture to match the exact native entries found in the screenshot
path. The fixture should include the real pinned release URL but fake token
values such as `xoxp-test`, never real credentials.

### 2. Fix only canonicalization at the native boundary

Keep the fix in `canonical_definition()` and the small helper functions it owns.
The target is a narrow canonical stdio definition:

```json
{
  "command": "npx",
  "args": [
    "-y",
    "https://github.com/davidvasandani/slack-mcp-server/releases/download/v1.3.0-vk.2/slack-mcp-server-vk-1.3.0-vk.2.tgz",
    "--transport",
    "stdio"
  ],
  "env": { "SLACK_MCP_XOXP_TOKEN": "xoxp-test" }
}
```

Implementation rules:

- Normalize only executor representation differences that are semantically
  equivalent for stdio MCP servers.
- Preserve argument order exactly.
- Preserve command string exactly after native parser decoding; do not split or
  shell-parse commands unless the failing fixture proves that an existing
  supported executor persists the command that way.
- Preserve env key names exactly.
- Preserve env values in fingerprints. Differences such as
  `xoxp-one` versus `xoxp-two` must still conflict.
- Do not redact Slack native env values in the backend comparison path; use fake
  values in tests instead.
- Do not add Slack-specific equality exceptions unless a general stdio
  canonicalization rule cannot safely express the observed native difference.

If the failing fixture shows a field that should be ignored, document why it is
executor metadata rather than launch semantics before dropping it from the
fingerprint.

### 3. Preserve real conflict detection

Add table-style regression coverage on the same `reconcile_snapshots()` path.
Each case should change only one Slack semantic field across otherwise
equivalent profiles and assert one conflict:

- command differs,
- one arg differs,
- `--transport stdio` is changed or removed,
- release artifact URL differs,
- env variable name differs,
- fake token value differs.

These tests are the main guard against over-normalization.

### 4. Verify materialization round trip

Add a write-path test that takes the reconciled Slack definition, builds a
`SharedMcpWriteRequest` assigning Codex, Claude Code, Gemini, and Grok, runs
`plan_servers_for_executor()` for each executor, then reconciles the resulting
native entries again.

Assert the second read has zero conflicts. Also assert the materialized entries
are valid native shapes for each executor:

- Codex and Grok entries remain TOML-compatible stdio objects under
  `mcp_servers`.
- Claude Code and Gemini entries remain JSON-compatible stdio objects under
  `mcpServers`.
- The Slack command, pinned release arg, `--transport stdio`, and
  `SLACK_MCP_XOXP_TOKEN` env key survive unchanged.

### 5. Keep the pinned Slack contract unchanged

Do not edit `crates/executors/default_mcp.json`,
`SLACK_MCP_FORK_TAG`, `SLACK_MCP_LAUNCHER_SHA256`, or
`docs/integrations/mcp-server-configuration.mdx` unless a test proves they are
wrong. The expected outcome is no documentation update because the supported
Slack launch contract and user-facing behavior remain the same except that a
false conflict disappears.

## Data Model

No `data-model.md` is needed. The feature changes no persisted entities,
database schema, generated TypeScript types, or saved API payload shape. It only
adjusts how existing native MCP JSON/TOML entries are interpreted before
fingerprinting.

## Contracts

No `contracts.md` is needed. The HTTP API and generated TypeScript contracts
should remain unchanged. The stable behavioral contract is the existing
`GET /api/mcp-config/shared` read model and `POST /api/mcp-config/shared`
materialization path:

- logically equivalent same-name Slack entries reconcile into one shared server,
- genuinely different same-name entries remain conflicts,
- writes materialize the canonical server into executor-native config shapes.

## Research Notes

See `./research.md` for the repository evidence, rejected alternatives, and test
strategy.

## Constitution Check

| Principle | Plan compliance |
| --- | --- |
| I. Clarity over cleverness | Fix the comparison boundary directly and keep the normalization rules explicit. |
| II. Test the contract | Start with the screenshot-shaped regression, then add negative conflict cases and a write/read round trip. |
| III. Small, reversible steps | Scope is limited to shared MCP canonicalization tests and the minimal helper change needed to pass them. |
| IV. Shared-component boundaries are law | Not applicable; no frontend shared package change is planned. If implementation proves UI changes are required, both local and remote web blast radius must be reassessed. |
| V. Remote mutations are transactional and txid-covered | Not applicable; no remote server mutation or ElectricSQL path is planned. |
| VI. Don't rebuild what shipped | Reuse existing shared MCP read, canonicalization, fingerprinting, and materialization helpers rather than adding a parallel Slack comparison path. |
| VII. Workspace breadcrumbs preserve issue identity | Not applicable; no workspace breadcrumb or issue navigation change is planned. |
| VIII. Managed tools are pinned, verified, and user-owned | Preserve the pinned Slack launch artifact and user-managed token boundary; do not write or log real Slack credentials. |
| IX. External agent protocols are defensive contracts | Keep executor-native MCP config adapters as the protocol boundary and preserve stable executor behavior across Codex, Claude Code, Gemini, and Grok. |
| X. Dialogs hold provisional state; containers hold confirmed state | Not applicable; no settings dialog state model change is planned. |
| XI. Diagnostics are evidence, not decoration | No diagnostic text changes are planned; any validation failure handoff must report exact commands and reasons without exposing secrets. |
| XII. Asynchronous handoffs have one authoritative owner | Not applicable; no queued/asynchronous lifecycle handoff change is planned. |
| XIII. Vendor config files are edited, never owned | Materialization must continue to touch only managed MCP entries in external agent config files and preserve unrelated native config content. |
| XIV. Repository verification is worktree-safe | Verification includes focused Rust tests, required formatting, and backend check when dependencies are installed; skipped commands must be documented. |
| XV. Destructive operations fail safe and are loud | Not applicable; no delete/reset/overwrite of worktrees or repositories is planned. |
| XVI. Bundled third-party entries install what they advertise | The pinned Slack fork URL, tag, digest, metadata link, docs, and integrity tests remain synchronized and must not be weakened. |
| XVII. Live capability state is confirmed and atomic | Do not report live MCP capability refresh from disk-only reconciliation; this task is limited to persisted shared MCP configuration comparison/materialization. |

No deviations.

The final knowledge-base update and commit are explicit user requirements for
this task. The implementation handoff must perform them directly rather than
introducing a second approval gate.

## Verification Plan

Run, in order:

1. `cargo test -p executors shared_mcp_config --lib`
2. `cargo test -p executors slack_preconfigured_server_matches_the_documented_stdio_contract`
3. `cargo test -p executors slack_preconfigured_server_pins_an_immutable_fork_artifact`
4. `pnpm run format`
5. If dependencies are installed in the worktree, `pnpm run backend:check`
6. Update the relevant `docs/knowledge-base/` entry with reusable knowledge
   learned during implementation and commit the implementation plus knowledge
   update together.

Do not run the ignored network digest test as part of normal implementation
verification. It is only required when re-pinning the Slack launcher, which is
out of scope for this task.

## Risks & Dependencies

- The screenshot's exact native entries may include a shape not obvious from
  checked-in defaults. Mitigation: make the first test fixture match persisted
  executor config shapes, not only `default_mcp.json`.
- Over-normalization could hide real config drift. Mitigation: add one negative
  conflict test per semantic Slack field.
- Token handling is sensitive. Mitigation: use fake values in tests and keep
  value-sensitive fingerprints rather than introducing Slack token redaction in
  the comparison path.
- Formatting/check commands may require `pnpm install --frozen-lockfile` in a
  fresh worktree before full verification.
