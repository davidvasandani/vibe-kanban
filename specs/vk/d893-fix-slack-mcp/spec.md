# Feature Specification: Fix Slack MCP Native-Configuration Conflict

**Feature dir**: `specs/vk/d893-fix-slack-mcp/`
**Status**: Implemented
**Task**: `d893-fix-slack-mcp`

## Summary

Vibe Kanban should recognize the bundled Slack MCP server as the same shared
server when its native definitions are logically equivalent across Codex,
Claude Code, Gemini, and Grok. The current shared MCP settings flow can report a
false native-configuration conflict for Slack because executor-specific native
serialization, especially Codex's shape versus the other assigned executor
shapes, is treated as a meaningful difference.

The fix must remove only that false conflict. Real differences in Slack command,
arguments, transport, launcher artifact, or credential value must still be
reported as conflicts so users do not accidentally overwrite a working profile
with an incompatible one.

Clarifications are recorded in
`specs/vk/d893-fix-slack-mcp/clarifications.md`. The supplied screenshot's
specific defect is the `slack` conflict split between Codex and the grouped
Claude Code/Gemini/Grok definition.

## Why

Shared MCP configuration is meant to let users manage one logical server across
multiple coding-agent profiles. A false Slack conflict breaks that promise: it
forces unnecessary conflict resolution, makes the bundled server look unsafe or
inconsistent, and creates a risk that saving the wrong variant rewrites several
native executor configs.

Slack is also a special bundled server because Vibe Kanban intentionally ships a
pinned `davidvasandani/slack-mcp-server` fork release rather than the upstream
npm package. That fork provides attachment retrieval behavior used by Vibe
Kanban workflows. Fixing reconciliation must not weaken the pinned release URL,
version, digest, metadata, stdio launch contract, or operator escape hatch that
protect this executable-source contract.

## User Stories

- As a user with Slack assigned to several agents, I want Vibe Kanban to show one
  shared Slack server when the agents are using equivalent native definitions.
- As a user resolving MCP conflicts, I want real Slack differences to remain
  visible so I can choose deliberately instead of silently merging incompatible
  configs.
- As a user relying on Slack attachments, I want the bundled Slack server to keep
  launching the pinned fork that provides attachment retrieval.
- As an operator, I want the fix to stay inside Vibe Kanban's shared MCP
  handling and bundled catalog, without changing unrelated services or
  deployment modules.

## Functional Requirements

- FR-1: Reading logically equivalent bundled Slack stdio definitions from Codex,
  Claude Code, Gemini, and Grok native config files MUST reconcile to one shared
  MCP server.
- FR-2: Equivalent Slack definitions MUST NOT produce a native-configuration
  conflict solely because one executor stores stdio fields in an
  executor-specific native shape.
- FR-3: Reconciliation MUST preserve genuine cross-profile conflict detection.
  Slack definitions with meaningfully different commands, argument lists,
  transports, launcher artifacts, environment variable names, or credential
  values MUST continue to produce a conflict, except that the exact former
  bundled `slack-mcp-server@latest` template MUST migrate to the current pinned
  fork definition.
- FR-4: Credential comparison MUST remain value-sensitive for configured Slack
  credentials, while UI, logs, diagnostics, and tests MUST NOT expose real token
  values.
- FR-5: The reconciled Slack server MUST preserve the stdio launch contract:
  command `npx`, the pinned GitHub release tarball argument, `--transport
  stdio`, and the `SLACK_MCP_XOXP_TOKEN` environment variable.
- FR-6: The bundled Slack catalog entry MUST continue to point at the
  `davidvasandani/slack-mcp-server` fork release asset, not upstream
  `@latest`, a mutable npm install target, or an unpinned artifact.
- FR-7: The synchronized Slack fork contract MUST remain intact: catalog URL,
  fork release tag, recorded launcher SHA-256, metadata link, and user-facing
  documentation must agree whenever any one of them changes.
- FR-8: Existing Slack pinned-launcher and catalog-shape integrity checks MUST
  remain valid and must not be weakened or skipped to make reconciliation pass.
- FR-9: Saving a reconciled shared Slack server MUST materialize valid native
  definitions for every assigned supported executor.
- FR-10: Shared MCP reconciliation MUST use the same backend read and
  canonicalization path that powers the settings page, not a frontend-only or
  catalog-only comparison.
- FR-11: The behavior MUST apply to Vibe Kanban's supported coding-agent
  profiles only. No homelab service, external Slack deployment, or unrelated MCP
  server behavior is in scope.
- FR-12: User-facing integration documentation outside
  `docs/knowledge-base/` MUST be updated only if the implementation changes the
  supported Slack launch contract, operator escape hatch behavior, pinned
  artifact metadata, or user-facing reconciliation behavior.
- FR-13: The final implementation MUST update the project knowledge base with
  reusable implementation knowledge from the task and commit the implementation
  plus that knowledge-base update together. The user has already required this
  final knowledge-base update and commit, so no additional approval gate is
  needed.

## Non-functional Requirements

- NF-1: Normalization must be narrow and explainable. It may treat
  executor-specific representations of the same stdio definition as equivalent,
  but must not collapse unrelated MCP fields or unknown semantic differences.
- NF-2: The fix must avoid logging, committing, or displaying Slack tokens or any
  other secret environment values.
- NF-3: The existing shared MCP API and generated TypeScript contracts should
  remain unchanged unless implementation proves a contract change is necessary.
- NF-4: Regression coverage should exercise native representations that resemble
  persisted executor config files rather than only comparing `default_mcp.json`,
  including TOML-equivalent `mcp_servers` entries for Codex/Grok and
  JSON-family `mcpServers` entries for Claude Code/Gemini.
- NF-5: The implementation should be focused enough that failures can be traced
  to shared MCP canonicalization/materialization or bundled Slack configuration,
  not to broad MCP rewrites.

## Out of Scope

- Replacing the pinned Slack fork with the upstream Slack MCP package, an npm
  `@latest` target, or any other mutable launcher.
- Changing Slack OAuth, token provisioning, Slack workspace permissions, or
  Slack API behavior.
- Changing homelab deployment configuration, including
  `homelab/modules/vibe-kanban-rebuild.nix`, unless read-only investigation
  proves that repository deployment configuration is part of the Vibe Kanban
  defect.
- Changing unrelated bundled MCP servers or shared MCP conflict semantics.
- Building a new MCP settings UI flow or a new frontend-only conflict resolver.
- Editing generated TypeScript files directly.

## Acceptance Criteria

- [x] A regression test reproduces the screenshot's false Slack conflict for
      `slack`, split as Codex versus Claude Code/Gemini/Grok, and passes with
      zero conflicts after the fix.
- [x] The same test path still reports a conflict when Slack command, arguments,
      transport, launcher artifact, environment variable name, or token value
      differs across profiles.
- [x] The bundled Slack server still uses the pinned
      `davidvasandani/slack-mcp-server` release artifact and still exposes the
      documented `SLACK_MCP_XOXP_TOKEN` stdio contract.
- [x] Existing pinned Slack fork integrity tests, including shape and digest
      checks, remain valid.
- [x] Saving the reconciled Slack server writes valid native configuration for
      Codex, Claude Code, Gemini, and Grok without introducing a conflict on the
      next read.
- [x] Focused backend tests for shared MCP read/canonicalization/materialization
      pass.
- [x] Repository formatting is run before completion, and relevant backend
      checks pass or any inability to run them is documented.
- [x] The final implementation includes the required knowledge-base update and
      commit, with no additional approval gate before writing the knowledge-base
      entry.

## Assumptions

- The defect is caused by Vibe Kanban's shared MCP native-definition
  reconciliation boundary retaining the former bundled Slack template in some
  profiles, not by Slack credentials or an external Slack service outage.
- Codex and the JSON-family executors can validly serialize the same stdio
  server in different native shapes.
- The current pinned fork version is `v1.3.0-vk.2`; changing that version is not
  required for this task unless investigation proves the pin itself is defective.
- Preserving credential-sensitive conflict detection is more important than
  hiding differences behind broad normalization.
