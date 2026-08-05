# Feature Specification: MCP Identifier and Display-Label Separation

**Feature dir**: `specs/vk/a5f8-concat-repeating/`
**Task id**: `vk/a5f8-concat-repeating`
**Status**: Clarified

## Summary

Allow human-readable MCP names such as “Atlassian Rovo” to appear throughout
Vibe Kanban without using those labels as protocol identifiers. MCP configuration
must save with client-compatible keys while retaining friendly names in the
management UI, eliminating a generic class of save failures for names containing
spaces or punctuation.

## User Stories

- As a user adding a suggested MCP, I want it to save successfully without
  manually translating its friendly name into a machine identifier.
- As a user managing MCPs, I want friendly product names in the UI while still
  being able to see the exact identifier used by coding agents.
- As a user with existing MCP configuration and authentication state, I want a
  no-op edit/save to preserve the operational identity of every server.
- As an operator diagnosing invalid configuration, I want an actionable error
  that distinguishes an invalid identifier from a valid display label.

## Functional Requirements

- FR-1: Every MCP server written to an external coding-agent configuration must
  have a non-empty identifier matching `^[a-zA-Z0-9_-]+$`.
- FR-2: The system must model a server's protocol identifier separately from
  its optional human-readable display label.
- FR-3: A suggested/catalog MCP must use its stable catalog key as its preferred
  identifier and its catalog metadata name as its display label.
- FR-4: If an incoming suggested entry lacks a safe identifier, the system must
  derive one using the same deterministic normalization rule presented by
  validation errors.
- FR-5: Identifier derivation must not overwrite an existing server. A collision
  must be surfaced before persistence and the user must be able to choose a
  distinct identifier.
- FR-6: The MCP list and edit surface must render the display label when present,
  with the identifier available as secondary identity; entries without labels
  must continue to render their identifier.
- FR-7: Add, edit, rename, delete, assign, test, authenticate, disconnect,
  refresh, merge, and conflict-resolution operations must address servers by
  identifier rather than display label.
- FR-8: Display labels must survive the confirmed draft's save/reload cycle and
  JSON-mode round trip without being placed into unsupported fields in external
  coding-agent definitions.
- FR-9: Existing safe native configurations must preserve their identifiers and
  executable definitions during a no-op load/save.
- FR-10: Existing unsafe native keys must remain visible with an actionable
  validation state and must not be silently renamed during read.
- FR-11: Both the client form and authoritative server write boundary must reject
  manually supplied unsafe identifiers.
- FR-12: All identifiers and normalized collisions must be validated before any
  per-agent configuration file is written.

## Out of Scope

- Relaxing coding-agent identifier grammar.
- Automatically migrating existing unsafe native keys.
- Changing MCP transports, commands, credentials, OAuth providers, or catalog
  installation sources.
- Adding display metadata to an external client's native MCP definition unless
  that client explicitly supports it.
- Deployment changes or changes to any service outside Vibe Kanban.

## Acceptance Criteria

- [ ] Adding “Atlassian Rovo” produces the identifier `atlassian_rovo` (or its
      safe catalog key), displays “Atlassian Rovo,” and saves successfully.
- [ ] A second fixture whose label contains punctuation also saves under a safe
      identifier while retaining its original label.
- [ ] Reloading after save retains the friendly label and exact identifier.
- [ ] Native coding-agent configuration contains the safe identifier and no
      unsupported display-only field.
- [ ] Testing and authentication requests use the safe identifier, not the
      friendly label.
- [ ] Two suggestions that resolve to the same identifier cannot overwrite one
      another and persistence does not begin.
- [ ] A manually entered unsafe identifier is rejected in the dialog and by the
      backend write API with a safe suggestion.
- [ ] A no-op round trip of an existing safe server preserves its identifier,
      definition, assignments, and credential placeholders.
- [ ] An unsafe pre-existing native key is reported rather than auto-renamed.
- [ ] Focused backend and frontend regression tests, generated-type checks,
      formatting, and applicable repository checks pass.

## Open Questions

None. See [`clarifications.md`](clarifications.md).
