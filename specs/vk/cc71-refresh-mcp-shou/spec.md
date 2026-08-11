# Feature Specification: Refresh Active Remote MCP Snapshots

**Feature dir**: `specs/vk/cc71-refresh-mcp-shou/`
**Status**: Draft

## Summary

Make **Refresh MCP** on an active Codex session adopt the latest MCP definitions
owned by Vibe Kanban settings before asking Codex to reload its MCP servers.
Today, remote executions reload the stale snapshot that was materialized when
the session started, so settings changes appear to refresh successfully without
changing the session's available tools.

## User Stories

- As a user who changes an MCP definition, I want an active remote Codex session
  to adopt the change when I click **Refresh MCP** so that I can use the latest
  tools without losing the conversation.
- As a user monitoring refresh, I want the UI to distinguish work in progress,
  success, partial success, contention, unsupported refresh, and failure so that
  I know what the live session actually adopted.
- As an operator, I want materialization and reload failures to identify the
  failing phase without exposing secrets so that I can diagnose them safely.

## Functional Requirements

- FR-1: Refresh must resolve the latest settings-owned MCP server definitions
  for the active session's executor and profile.
- FR-2: Refresh must replace only the MCP-definition portion of the active
  execution's scoped Codex configuration before requesting a Codex MCP reload.
- FR-3: The replacement must be atomic: readers observe either the complete old
  MCP definition set or the complete new set.
- FR-4: Refresh must preserve unrelated Codex configuration, authentication,
  installed skills, history, conversation state, and other session state.
- FR-5: Refresh must remain scoped to the target execution so concurrent
  sessions cannot read or overwrite one another's snapshots.
- FR-6: Added, updated, disabled, and removed MCP definitions must be reflected
  in the refreshed snapshot.
- FR-7: Codex reload must begin only after the new snapshot has been materialized
  successfully.
- FR-8: A successful result must be based on Codex's per-server reload status,
  not solely on a successful configuration write.
- FR-9: Results must distinguish pending, refreshed, partially refreshed, busy,
  unsupported, materialization failure, and Codex reload/bootstrap failure.
- FR-10: Diagnostics and logs must identify the failing phase and configured
  server where applicable without exposing environment values, tokens,
  authenticated URLs, or secret-bearing arguments.
- FR-11: If safe in-place rematerialization is unavailable, the action must not
  claim a successful refresh and must explicitly offer or initiate the supported
  continuation/restart behavior with a fresh snapshot.
- FR-12: Refresh must coordinate concurrent requests for the same execution so
  they cannot interleave snapshot writes and reloads.
- FR-13: After a successful refresh, MCP initialization and tool listing through
  the worker must expose the refreshed server set while the existing Codex
  conversation remains usable.

## Out of Scope

- Changing MCP ownership away from Vibe Kanban settings.
- Editing user-global Codex configuration as part of a session refresh.
- Changes to services or deployment modules outside the Vibe Kanban service.
- Treating an independent MCP probe as proof that the active Codex process
  adopted the refreshed configuration.

## Acceptance Criteria

- [ ] A remote session created from settings snapshot A adopts snapshot B after
      settings change and **Refresh MCP**, without starting a new conversation.
- [ ] Snapshot B can add, update, disable, and remove definitions relative to A,
      and each change is reflected after refresh.
- [ ] The execution-scoped `config.toml` MCP section is replaced atomically
      before Codex receives `config/mcpServer/reload`.
- [ ] Non-MCP configuration, authentication, skills, history, and session state
      are byte-for-byte or behaviorally preserved as appropriate.
- [ ] Concurrent executions with different scoped homes cannot affect each
      other's MCP snapshots.
- [ ] Concurrent refresh attempts for one execution produce a truthful busy or
      serialized outcome rather than interleaved writes.
- [ ] UI tests cover pending, refreshed, partially refreshed, busy, unsupported,
      materialization-failed, and reload/bootstrap-failed outcomes.
- [ ] Errors distinguish materialization from reload/bootstrap and do not reveal
      secret values.
- [ ] An end-to-end regression test proves `tools/list` exposes B after refresh
      while conversation state established under A remains intact.
- [ ] A worker-side smoke test performs MCP initialization and `tools/list`
      against the refreshed scoped configuration.

## Open Questions

None. See `clarifications.md` for the resolved decisions.
