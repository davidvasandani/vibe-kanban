# Feature Specification: List all MCP conversation messages

**Feature dir**: `specs/vk/29d8-vk-list-all-mess/`
**Status**: Draft

## Summary

Provide a `list_all_messages` Vibe Kanban MCP tool so an orchestrator can read
the complete normalized message projection for one selected coding-agent turn,
including context that falls outside `list_recent_messages`' bounded tail.

## User Stories

- As an orchestrator, I want to read every available normalized message for an
  execution so that I can recover earlier decisions before sending a follow-up.
- As an orchestrator, I want to target a session in the same way as the recent
  reader so that I do not have to discover the latest coding-agent execution.
- As an operator, I want all-message reads to honor workspace scope and existing
  normalization safeguards so that the added visibility does not bypass access
  boundaries or expose raw logs.

## Functional Requirements

- FR-1: The MCP server exposes a read-only tool named `list_all_messages`.
- FR-2: The tool accepts a session identifier or an execution identifier and
  selects the same execution that `list_recent_messages` would select.
- FR-3: The tool rejects requests without either target identifier.
- FR-4: The tool verifies access to the owning workspace before reading data.
- FR-5: The tool returns every message in Vibe Kanban's settled normalized
  projection for the selected execution, oldest first, without applying the
  recent reader's 100-message response cap.
- FR-6: The tool accepts the existing optional role filter and returns only
  matching user, assistant, system, or tool messages.
- FR-7: Its response preserves the existing message identity, timestamps,
  status, exit code, final assistant message, per-entry text truncation, and
  normalized-history omission notice behavior.
- FR-8: A successful all-message response reports that no additional messages
  remain within the selected normalized projection.
- FR-9: Existing `list_recent_messages` inputs, limits, and outputs remain
  unchanged.
- FR-10: Scoped orchestrator MCP mode exposes the new tool.
- FR-11: Project documentation distinguishes bounded recent reads from complete
  normalized-projection reads and discloses the legacy oversized-history bound.

## Out of Scope

- Combining several executions into one session transcript.
- Returning raw logs or untruncated message bodies.
- Removing the historical normalization safety bound for legacy cache misses.
- Frontend or non-Vibe-Kanban service changes.

## Acceptance Criteria

- [ ] Tool discovery includes `list_all_messages` in scoped orchestrator mode.
- [ ] Given more than 100 normalized messages, `list_all_messages` returns all
      available entries in chronological order and `has_more: false`.
- [ ] The same fixture read through `list_recent_messages` remains capped and
      returns the newest entries with truthful `has_more`.
- [ ] Session and execution targets enforce the owning workspace scope.
- [ ] Role filtering, message truncation, and `final_message` match the existing
      recent-message semantics.
- [ ] Focused MCP/server tests, Rust formatting, and relevant checks pass.
- [ ] Independent Codex review reports no significant findings.

## Open Questions

None. See `clarifications.md` for the resolved decisions.
