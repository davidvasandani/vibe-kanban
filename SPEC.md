# Shared MCP Server Configuration — Technical Specification

## Problem

MCP server settings are currently edited one coding-agent configuration at a time. The same logical server must therefore be entered and maintained repeatedly for Claude, Codex, Gemini, and other MCP-capable agents, even though only the agent-specific serialization differs. This creates duplicate work and lets equivalent configurations drift.

## Goal

Redesign MCP settings around a single logical server definition that can be assigned to one or more MCP-capable agent profiles. A user enters or edits a server once, selects its target profiles, and Vibe Kanban materializes the appropriate representation in every selected agent's native configuration file.

## Scope

- Present a unified MCP server list rather than requiring the user to select one agent before viewing or editing servers.
- Let each logical server be shared with one or more MCP-capable agent profiles.
- Translate a logical server definition through each target agent's existing MCP codec/config strategy when writing native files.
- Preserve the form editor, raw JSON escape hatch, preconfigured server choices, connectivity testing, and OAuth connection flow where they remain meaningful.
- Reconcile existing per-agent MCP configurations into the unified view without silently deleting or overwriting unrelated configuration.
- Keep unsupported agents unavailable as assignment targets.

## Functional Requirements

1. The MCP settings page loads MCP configurations for all MCP-capable agent profiles and displays a consolidated list of logical servers.
2. Each server row and edit dialog shows the profiles to which the server is assigned.
3. Creating a server requires a unique name, a valid MCP configuration, and at least one compatible target profile.
4. Saving applies each server to every selected profile and removes it from profiles that were explicitly unassigned, while preserving other agent configuration fields.
5. Editing a shared server updates all assigned profiles in one save operation.
6. Deleting a shared server removes it from every assigned profile after the existing confirmation interaction (or an equivalent explicit action).
7. If existing profiles contain the same server name with equivalent normalized configuration, they are represented as one shared server with multiple assignments.
8. If existing profiles contain the same name with incompatible definitions, the UI must surface the conflict and avoid silently choosing one definition. The user can resolve it by editing/renaming or selecting an authoritative definition and assignments.
9. Agent-specific formats remain valid. The backend must use each executor's existing MCP configuration metadata and file codec when reading and writing.
10. Connectivity tests identify both the logical server and the profile used for the probe. Testing may cover all assignments or a selected assignment, but results must not be presented as applying to profiles that were not tested.
11. OAuth credentials written by a connection flow must be refreshed into the unified state so a later save does not erase them. OAuth remains scoped to the native profile configuration in which the token is stored.
12. A partial write failure must be reported per profile. The operation must avoid claiming overall success when any selected profile was not updated.

## API and Data Model

Introduce a shared-settings representation at the API boundary:

- `SharedMcpServer`: stable logical identifier/name, canonical server definition, assigned profile/executor identifiers, and optional conflict/source metadata.
- A read endpoint returns supported profiles, consolidated servers, conflicts, and relevant native config paths.
- A write endpoint accepts the complete desired shared-server state (or explicit upsert/delete operations) and fans changes out to native configs using existing `McpConfig` metadata.
- Test and OAuth endpoints accept enough assignment context to resolve the correct native agent config.

The implementation may persist the logical registry in Vibe Kanban assets or derive it from native files, provided native external edits remain discoverable and reconciliation is deterministic. Native agent configuration files remain the runtime source consumed by agent CLIs.

## Compatibility and Migration

- Existing native MCP entries must appear on first load; no manual migration is required.
- Existing API behavior may remain temporarily for compatibility, but the redesigned UI uses the shared endpoints.
- Unknown/custom JSON properties must round-trip for the profile(s) whose codec cannot represent them in the form editor.
- Names that occur in only one profile become logical servers assigned to that profile.
- The bundled Vibe Kanban MCP entry and preconfigured catalog continue to work.

## UX Requirements

- The primary unit is an MCP server card, not an agent selector.
- Cards summarize transport, assignment count/profile names, and per-profile test state.
- Add/edit interactions include a multi-select list of compatible profiles.
- The interface explains conflicts and partial failures next to the affected server/profile.
- Unsaved-change protection covers definition and assignment changes.
- Text is added to the English locale and kept structurally compatible with the project's translation workflow.

## Validation and Acceptance Criteria

- A user can add one server, assign it to at least two different MCP-capable agents, save once, and observe valid entries in both native config files.
- Changing the shared definition and saving once updates all assigned native files.
- Unassigning one profile removes only that profile's entry.
- Existing identical entries consolidate; incompatible same-name entries are flagged without data loss.
- Unsupported profiles cannot be selected.
- Backend unit tests cover consolidation, conflicts, assignment diffs, preservation of unrelated config, and partial errors.
- Frontend tests cover shared-state transformation and assignment editing where practical.
- Generated shared types are regenerated from Rust sources rather than edited manually.
- Formatting and focused frontend/backend checks pass.

## Non-Goals

- Running one MCP server process shared concurrently by multiple agent processes.
- Synchronizing MCP settings between different Vibe Kanban machines or users.
- Replacing the native MCP configuration formats used by third-party agent CLIs.
- Generalizing this work into a universal settings-sharing framework for non-MCP agent settings.

## Risks and Open Questions for SpecKit Clarification

- Whether "profiles" means base executor types, named executor configurations, or both; current MCP endpoints key by `BaseCodingAgent` even though the UI uses profile terminology.
- Whether a durable canonical registry is needed or native files should remain the only persisted source.
- How to translate definitions between agents when their supported transports or custom fields differ.
- Whether multi-file saves require rollback/transaction semantics or explicit per-profile partial-success recovery.
- How OAuth tokens should behave when the same remote MCP is assigned to several agents with different native credential schemas.
