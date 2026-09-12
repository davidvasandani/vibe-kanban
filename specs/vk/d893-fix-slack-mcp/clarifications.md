# Clarifications: Fix Slack MCP Native-Configuration Conflict

**Feature dir**: `specs/vk/d893-fix-slack-mcp/`
**Date**: 2026-07-30
**Status**: Resolved from supplied screenshot and repository evidence

## Evidence Reviewed

- Supplied screenshot:
  `../.vibe-attachments/774d28e4-ac88-4c7a-a9d5-f50c1b7a3cbe_cleanshot_20260730_at_1351022x.png`
- Parent task brief: `../SPEC.md`
- Parent prior knowledge: `../PRIOR_KNOWLEDGE.md`
- Current feature spec: `specs/vk/d893-fix-slack-mcp/spec.md`
- Shared MCP implementation: `crates/executors/src/shared_mcp_config.rs`
- MCP catalog/adapters/tests: `crates/executors/src/mcp_config.rs`
- Bundled catalog: `crates/executors/default_mcp.json`
- User-facing MCP documentation:
  `docs/integrations/mcp-server-configuration.mdx`
- Knowledge base:
  `docs/knowledge-base/shared-mcp-configuration.md`
  `docs/knowledge-base/forked-mcp-server-packaging.md`

## Decisions

### Q1: What exact defect must be reproduced?

**Decision**: Reproduce the screenshot's shared MCP read-model conflict for the
server named `slack`, where Codex appears as one native-definition variant and
Claude Code, Gemini, and Grok appear as a second variant.

**Rationale**: The screenshot explicitly reports: "MCP server `slack` has
different definitions across assigned profiles" with choices for "Use Codex
definition" and "Use Claude Code, Gemini, Grok definition." The task brief names
the same Codex-versus-other-profile mismatch.

**Spec impact**: Acceptance criteria should require this exact grouping to be
covered by a backend regression test.

### Q2: Should Slack token values be ignored or redacted during conflict detection?

**Decision**: No. Configured Slack credential values remain part of the
normalized comparison, while tests and diagnostics must use placeholders or fake
values and must not expose real tokens.

**Rationale**: Both the parent task and current spec require genuine credential
differences to remain conflicts. The shared MCP read path currently computes
fingerprints from canonical definitions before UI redaction, and only shared
gateway bearer capabilities have special redaction/preservation behavior.

**Spec impact**: Preserve FR-3/FR-4 as written. Tests for token differences
should use fake strings such as `xoxp-one`/`xoxp-two`.

### Q3: Which layer owns the fix?

**Decision**: The fix belongs in the backend shared MCP native
read/canonicalization/materialization path, primarily
`crates/executors/src/shared_mcp_config.rs`, with catalog-adapter evidence from
`crates/executors/src/mcp_config.rs` as needed. It must not be solved in the
frontend conflict dialog.

**Rationale**: `docs/knowledge-base/shared-mcp-configuration.md` states that
native executor configuration files are the source of truth and that
`GET /api/mcp-config/shared` normalizes native entries, merges equivalent
same-name definitions, and reports conflicts. The screenshot is rendered from
that read model, so a frontend-only change would hide rather than fix the false
conflict.

**Spec impact**: FR-10 stands. Planning should start from backend
canonicalization tests, not UI state tests.

**Implementation discovery**: The exact mismatch was not TOML versus JSON.
Codex used the current pinned fork while Claude Code, Gemini, and Grok retained
the former bundled `slack-mcp-server@latest` entry. Their credential hashes
matched. The backend therefore owns a narrow historical-template migration,
not a broad executor-shape normalization.

### Q4: Is the bundled Slack launch contract changing?

**Decision**: No. Keep the current pinned fork release contract unchanged:
`npx`, `-y`, the
`https://github.com/davidvasandani/slack-mcp-server/releases/download/v1.3.0-vk.2/slack-mcp-server-vk-1.3.0-vk.2.tgz`
argument, `--transport stdio`, and `SLACK_MCP_XOXP_TOKEN`.

**Rationale**: `crates/executors/default_mcp.json`,
`crates/executors/src/mcp_config.rs`, and
`docs/integrations/mcp-server-configuration.mdx` already agree on
`v1.3.0-vk.2`, and the existing shape/digest tests are documented security
controls. The parent prior knowledge says the pin itself is not the likely
defect unless direct investigation proves otherwise; current repository evidence
does not prove that.

**Spec impact**: User-facing integration documentation updates are only required
if implementation changes the supported Slack launch contract, operator escape
hatch behavior, or the pinned artifact metadata. The final project
knowledge-base update remains required separately by the user.

### Q5: Which executor profiles are in scope for the regression?

**Decision**: The required regression scope is Codex, Claude Code, Gemini, and
Grok, matching the screenshot. The implementation may improve shared
canonicalization for the same stdio shape across other MCP-capable agents, but
that is incidental and must not broaden acceptance requirements or weaken
conflict semantics.

**Rationale**: The screenshot and parent task name only those four assigned
profiles. Repository configuration shows Codex and Grok read TOML under
`mcp_servers`, while Claude Code and Gemini use JSON-family `mcpServers`.

**Spec impact**: Tests should construct native snapshots that resemble those
persisted executor config entries, including Codex/Grok TOML-equivalent shapes
after repository TOML parsing has produced JSON values.

### Q6: Should homelab deployment configuration be changed?

**Decision**: No planned homelab changes. Treat
`homelab/modules/vibe-kanban-rebuild.nix` and other external deployment files as
out of scope unless implementation investigation later proves they are the
actual source of the false Vibe Kanban settings conflict.

**Rationale**: The parent task explicitly scopes homelab changes as conditional.
The screenshot is a Vibe Kanban shared MCP settings read-model conflict, and the
repository has a clear backend reconciliation path for that behavior.

**Spec impact**: Keep homelab listed as out of scope.

## Remaining Open Questions

None. Repository evidence safely resolves the material choices needed before
planning.
