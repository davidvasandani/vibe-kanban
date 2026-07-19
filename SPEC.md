# VK MCP Management UX Technical Specification

## Objective

Refine the MCP Servers settings experience to match the supplied management-interface references: a compact server inventory with clear status and actions, plus a focused add/edit modal. Move agent assignment controls out of the primary server list and into that modal so the main settings surface remains easy to scan.

## Scope

- Update the existing MCP settings frontend in `packages/web-core`.
- Preserve the current shared MCP data model, persistence APIs, authentication flows, connection testing, conflict handling, and JSON editing escape hatch.
- Present each configured MCP server as a concise management card with its name, transport, assignment summary, relevant connection/auth state, and explicit Test/Edit/Delete actions.
- Place supported-agent assignment controls in the add/edit MCP server dialog.
- Ensure add and edit flows validate that the server has a valid definition and at least one compatible agent assignment where required by the existing model.
- Keep the interface responsive and aligned with the new-design tokens and existing reusable UI primitives.
- Update user-facing localization strings and tests affected by the interaction change.

## Reference-derived UX requirements

1. The main section leads with an `MCP Servers` heading/count and short explanatory copy, with the add action visually separated from the inventory.
2. Server cards prioritize identity and operational state over configuration mechanics.
3. Card actions are plainly discoverable (Test, Edit, Delete; auth-specific actions where applicable).
4. The add/edit dialog contains server configuration and agent selection in one coherent form.
5. Agent compatibility is shown at selection time; incompatible agents cannot be assigned and expose the existing reason.
6. Editing and canceling do not mutate the saved draft until the dialog is submitted.
7. The dialog remains usable on narrow screens and with keyboard navigation.

## Functional requirements

- Opening Add initializes a fresh form and sensible default assignment state without changing the underlying draft.
- Opening Edit hydrates the definition and current assignments for that server.
- Submitting Add/Edit writes the complete server object, including assignments, to the local draft and closes the dialog.
- Canceling or closing discards all unsaved modal changes, including assignment changes.
- Renaming a server through Edit replaces the original entry without leaving a duplicate.
- The primary list no longer renders an assignment checkbox grid.
- Test results and connection/auth affordances remain available from the server card.
- JSON mode continues to round-trip the same `SharedMcpDraftServer[]` representation.
- Existing save/discard semantics remain unchanged: modal submission updates the local draft; the settings save bar persists it.

## Non-functional requirements

- Use the scoped new-design color, type, spacing, radius, and focus tokens.
- Reuse existing Button, Dialog, SettingsField, and related primitives.
- Do not alter generated shared TypeScript types directly.
- Preserve accessibility through associated labels, native checkbox behavior, button titles/visible labels, focus handling, and dialog semantics.
- Avoid backend/schema changes unless implementation proves they are necessary.

## Verification

- Add or update focused frontend tests for modal-local assignment editing, submit/cancel behavior, compatibility disabling, and list rendering.
- Run targeted tests and frontend type checking/linting for changed files.
- Run repository formatting before completion.
- Visually inspect the MCP settings section and add/edit dialog at desktop and narrow widths when a runnable local environment is available.

## Out of scope

- Changing MCP transport codecs, gateway behavior, OAuth semantics, backend endpoints, or shared configuration storage.
- Reproducing product-specific wording or features from the screenshots that are not represented by Vibe Kanban's current MCP model (for example enable/disable state or persisted tool-count telemetry).
- Removing advanced JSON editing or conflict resolution.
