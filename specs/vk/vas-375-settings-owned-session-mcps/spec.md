# Feature Specification: Settings-Owned MCPs in Every New Session

**Task**: VAS-375 follow-up
**Status**: Draft
**Scope**: Vibe Kanban remote execution and its homelab deployment configuration

## Problem

Vibe Kanban Settings stores complete, authenticated MCP definitions and assigns
them to executor profiles. Remote dispatch currently snapshots those settings
only for Codex. Other executors start from worker or repository configuration,
so a new Claude or Gemini session can miss the saved MCPs. A project-level
`homelab/.mcp.json` also defines the same Vibe Kanban endpoint under a different
identifier using environment placeholders; those placeholders are not created
when a user saves literal headers in Settings.

Users consequently see an MCP as saved and healthy in Settings while a newly
started session reports that its required environment variables are unset.

## User Outcomes

1. A new remote session sees the latest MCP definitions assigned to its selected
   executor profile.
2. Saved authenticated HTTP headers reach the session through the existing
   authenticated snapshot boundary without requiring equivalent environment
   variables in the repository.
3. Concurrent executions cannot overwrite or inherit one another's MCP config.
4. Repository and worker-global vendor configurations remain unchanged.
5. The homelab repository no longer supplies a competing Vibe Kanban MCP
   definition.

## Functional Requirements

- **FR-001**: The coordinator MUST attach the selected profile's native MCP
  server map to every remote execution whose executor supports MCP config.
- **FR-002**: A snapshot MUST identify its executor, remain bounded, participate
  in dispatch idempotency, and fail closed when it does not match the dispatched
  executor.
- **FR-003**: The worker MUST materialize a supplied snapshot in an
  execution-scoped native vendor config before starting the child process.
- **FR-004**: The worker MUST redirect only that execution to the scoped config
  while preserving access to unrelated home-directory runtime and
  authentication assets.
- **FR-005**: The worker MUST atomically update the scoped native config and MUST
  NOT mutate repository config or the worker's global vendor config.
- **FR-006**: Snapshot definitions, headers, tokens, authenticated URLs, and
  environment values MUST NOT appear in logs or diagnostics.
- **FR-007**: Codex's confirmed live-refresh path MUST continue to operate on its
  scoped config. Executors without confirmed live reload adopt settings at a
  fresh process boundary.
- **FR-008**: Execution cleanup MUST remove the scoped MCP configuration.
- **FR-009**: The Vibe Kanban deployment surface in `homelab/.mcp.json` MUST not
  define the Settings-owned Vibe Kanban MCP.
- **FR-010**: Existing settings persistence, profile assignment, native adapters,
  and unrelated MCP definitions MUST remain unchanged.

## Acceptance Scenarios

1. Given saved MCP definitions assigned to a Claude, Codex, or Gemini profile,
   when a new remote execution starts, its native config contains exactly that
   profile's server map.
2. Given an authenticated HTTP MCP saved with literal headers, when a new remote
   execution starts, the definition is usable without repository placeholder
   environment variables.
3. Given two simultaneous executions with different profiles, each reads only
   its own native MCP map and neither changes the worker-global config.
4. Given a mismatched or invalid snapshot, dispatch fails before the coding
   agent starts and the error contains no secret values.
5. Given an active Codex execution, a confirmed refresh replaces only its scoped
   MCP map and preserves unrelated Codex configuration and runtime assets.
6. Given the homelab repository after deployment cleanup, its project MCP config
   has no Vibe Kanban definition and all unrelated definitions are preserved.

## Non-Goals

- Creating environment variables from saved literal header values.
- Adding live MCP reload to executors that cannot confirm it.
- Moving MCP definition authority into Nix, repository files, or worker-global
  vendor configuration.
- Changing Cloudflare Access policy or the public MCP transport.
- Rotating credentials exposed in screenshots; that is an immediate operational
  action outside the code change.

## Success Measures

- Regression tests cover coordinator snapshot creation for non-Codex executors,
  executor-scoped worker materialization, concurrent isolation, cleanup, and the
  retained Codex refresh contract.
- Repository verification passes for the affected Rust workspaces.
- No competing Vibe Kanban entry remains in `homelab/.mcp.json`.
