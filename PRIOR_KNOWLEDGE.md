# Prior Knowledge: VK MCP Management UX

Task: `76d1-vk-mcp-ux`

The Vibe Kanban project knowledge base was searched via `docs/knowledge-base/INDEX.md` and topic-page text for MCP, settings, assignment, modal, and frontend guidance. Four pages directly inform this change.

## Shared MCP configuration

Source: `docs/knowledge-base/shared-mcp-configuration.md`

- The UI edits a logical shared server list, but native agent config files remain the actual source and write targets.
- Assignments target base executor types, not named variants or task overrides.
- Compatibility must be enforced in the frontend as well as the backend. Codex and Grok, for example, accept stdio and streamable HTTP but not legacy SSE.
- Saves materialize a complete logical server list across independent native profiles and can partially succeed, so the existing save/error reporting contract must remain intact.
- Gateway-backed authentication depends on assignment-level behavior. Removing assignments and disconnecting a gateway are not interchangeable operations.
- Redacted capabilities and refreshed gateway entries need the existing snapshot/hydration flow; a UI-only reorganization must not rewrite or reconstruct secret-bearing definitions.

## MCP connectivity testing

Source: `docs/knowledge-base/mcp-connectivity-testing.md`

- Vibe Kanban is primarily a config writer; connectivity checks are explicit, on-demand probes of saved native entries.
- Test results are keyed by both logical server and executor in the current shared flow. Moving assignment controls must preserve per-assignment status and testing behavior.
- Status must distinguish connected, failed, authentication-required, and unsupported cases rather than flattening them into a binary indicator.
- Existing results are invalidated by edits/saves so stale operational state is not presented as current.

## MCP OAuth Connect flow

Source: `docs/knowledge-base/mcp-oauth-connect.md`

- An `auth_required` test result drives the Connect flow; the frontend opens OAuth synchronously enough to avoid popup blocking, polls status, refreshes the disk snapshot, and re-tests the server.
- OAuth completion writes behind the UI. The refreshed on-disk entry must be merged into both the editable draft and original snapshot so later Save does not erase credentials.
- Loopback/manual completion and Cloudflare Access retry guidance are part of the existing card-level result UI and should remain reachable after the redesign.
- Connection identity depends on canonical server name/URL/assignment matching; modal editing must continue to pass complete existing definitions through the established helpers.

## Grok executor integration

Source: `docs/knowledge-base/grok-executor-integration.md`

- Agent transports differ, and frontend assignment filters must mirror backend compatibility checks.
- The current codecs are the authoritative frontend mechanism for deciding whether a server definition can be assigned to a profile.

## Implications for specification and planning

1. Treat this as a frontend information-architecture change, not a new MCP storage model.
2. Move assignment editing into modal-local state and commit it only with the rest of the modal form.
3. Keep tests, auth controls, and detailed per-executor failure information on or reachable from each compact server card.
4. Reuse `codecForAgent` compatibility checks and existing save/snapshot/auth functions; do not duplicate transport policy.
5. Test cancellation carefully because the current inline checkboxes mutate the draft immediately, while the requested modal must provide transactional editing.
