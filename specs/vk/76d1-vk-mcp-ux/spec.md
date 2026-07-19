# Feature Specification: MCP Management UX Redesign

**Feature dir**: `specs/vk/76d1-vk-mcp-ux/`
**Status**: Draft
**Task**: `vk/76d1-vk-mcp-ux`

## Summary

The current MCP Servers settings surface mixes server configuration mechanics
with agent-assignment checkboxes in the same scrollable list, making it hard to
scan operational state at a glance and awkward to manage assignments. This
feature redesigns the surface to match a compact management-card inventory: each
configured server shows its name, transport, assignment summary, and connection
state with explicit Test / Edit / Delete actions. Agent assignment controls move
out of the primary list and into the add/edit modal, where they live alongside
server configuration as part of a single, transactional form. The net effect is a
settings section that is easier to read, easier to operate, and impossible to
leave in a partially-edited state.

## Why

The existing inline checkbox grid mutates the shared draft immediately — a
violation of Principle X (dialogs hold provisional state; containers hold
confirmed state). Users currently have no clear boundary between "browsing server
state" and "changing assignments," and no cancel path that rolls back only the
assignment change without discarding the whole settings section. Moving assignment
editing into the modal gives every change a distinct commit action (Save) and
discard action (Cancel), fixes the transactional-state violation, and reduces
visual noise in the primary list.

## User Stories

- As a user managing MCP servers, I want to see each server as a concise card
  showing its name, transport, which agents use it, and its current connection
  state, so I can understand my MCP configuration at a glance without scrolling
  through controls I am not changing.
- As a user adding a new MCP server, I want a single dialog that lets me
  configure the server and choose which agents should use it, so I can complete
  the full setup in one place without returning to the list.
- As a user editing an existing server, I want the edit dialog to open pre-filled
  with the current configuration and assignments, so I can make targeted changes
  without re-entering data.
- As a user editing a server, I want canceling or closing the dialog to discard
  all my changes including assignment changes, so I never accidentally write a
  partial edit.
- As a user assigning agents in the modal, I want incompatible agents to be shown
  with a clear reason why they cannot be assigned to this server, so I understand
  the constraint without needing to look it up.
- As a user, I want to test, view connection status, and trigger OAuth Connect
  from the server card without opening the edit dialog, so operational actions
  remain accessible without entering an edit flow.

## Functional Requirements

- FR-1: The MCP Servers section MUST lead with a heading that includes the count
  of configured servers and a short explanatory sentence, with the Add button
  visually separated from the server inventory.
- FR-2: Each configured server MUST be rendered as a management card that
  displays: server name, transport type, a summary of which agents are assigned
  (e.g. "N agents" or a list of agent names), and current connection/auth state
  (connected, failed, auth-required, unsupported, or untested).
- FR-3: Each server card MUST expose at minimum three explicit actions: Test,
  Edit, and Delete. Auth-specific actions (Connect) MUST remain reachable from
  the card when the connection state is `auth_required`.
- FR-4: The primary server list MUST NOT render agent-assignment checkboxes or
  any inline assignment control.
- FR-5: Opening Add MUST initialize a blank form with sensible defaults and one
  compatible agent pre-selected by default (the first profile that supports the
  current transport), without mutating the underlying draft.
- FR-6: Opening Edit MUST hydrate the modal with the current saved configuration
  and current agent assignments for that server, without mutating the underlying
  draft.
- FR-7: The add/edit modal MUST contain both the server definition fields
  (name, transport, URL/command, headers, environment variables, etc.) and the
  agent assignment controls in one coherent form.
- FR-8: Agent assignment controls in the modal MUST use the existing
  `codecForAgent` compatibility check evaluated per profile at the time the form
  is rendered. Compatibility is derived dynamically from the current transport
  selection (not cached backend data), so it updates as the user changes the
  transport. Incompatible agents MUST be visible but not assignable, with the
  incompatibility reason displayed. For custom-JSON entries (where transport is
  not parseable by the form), all agents should be selectable with no
  compatibility filtering applied, since transport cannot be determined without
  parsing the raw JSON.
- FR-9: Submitting the modal MUST write the complete server object — definition
  plus assignments — to the local draft and close the dialog. No partial write is
  permitted.
- FR-10: Canceling or closing the modal (without submitting) MUST discard all
  modal-local changes, including any assignment changes, leaving the outer draft
  unchanged.
- FR-11: Renaming a server through the edit modal MUST replace the original entry
  under its new name without leaving a duplicate entry in the draft.
- FR-12: Test results and per-executor connection/auth state MUST remain
  accessible from the server card after the assignment controls move to the modal.
  Test results are keyed by `testKey(serverName, executor)` and stored in the
  section's `Record<string, SharedMcpAssignmentTestResult>` state; no state shape
  change is required.
- FR-13: The JSON editing escape hatch MUST continue to round-trip the same
  `SharedMcpDraftServer[]` structure it uses today.
- FR-14: Existing save/discard semantics at the settings level MUST be
  unchanged: modal submission updates the local draft; the settings save bar
  persists the draft.
- FR-15: The add/edit modal MUST validate that the server has a valid definition
  before submission, consistent with existing validation behavior. In addition,
  the modal MUST require at least one agent to be assigned before submission is
  permitted; an error message MUST be shown if the user attempts to submit with
  no assignments selected.
- FR-16: The OAuth Connect flow MUST remain reachable from the server card and
  MUST continue to merge the refreshed on-disk credential entry into both the
  editable draft and the original snapshot, so a subsequent Save does not erase
  credentials written by the OAuth callback.

## Non-functional Requirements

- NF-1: Use the new-design color, typography, spacing, radius, and focus tokens
  consistently throughout the redesigned surface.
- NF-2: Reuse existing primitives — Button, Dialog, SettingsField, and related
  shared components — rather than introducing new presentational components for
  one-off use.
- NF-3: Do not alter generated shared TypeScript types (`shared/types.ts`); use
  `generate-types` scripts if type changes are needed.
- NF-4: Preserve accessibility: every control must have an associated label,
  checkboxes must use native behavior, buttons must have visible labels or titles,
  focus must be managed into and out of the dialog, and the dialog must use
  correct ARIA dialog semantics.
- NF-5: The add/edit dialog MUST be operable at narrow viewport widths and with
  keyboard-only navigation.
- NF-6: Avoid backend or schema changes unless implementation reveals they are
  unavoidable; if required, record the reason in the plan.

## Out of Scope

- Changing MCP transport codecs, gateway behavior, OAuth semantics, backend
  endpoints, or the shared configuration storage format.
- Reproducing product-specific features visible in reference screenshots that
  have no counterpart in Vibe Kanban's current MCP model (e.g. per-server
  enable/disable toggle, persisted tool-count telemetry).
- Removing the advanced JSON editing escape hatch or conflict-resolution flow.
- Changes to the remote-web MCP settings surface unless they share components
  that must be updated to satisfy FR-4 or FR-9.

## Acceptance Criteria

- [ ] The MCP Servers section heading shows the server count and explanatory copy;
      the Add button is visually distinct from the server inventory.
- [ ] Each server card displays name, transport, agent-assignment summary, and
      connection state without rendering assignment checkboxes inline.
- [ ] Test, Edit, and Delete actions are plainly visible on each server card.
- [ ] Connect (OAuth) action is reachable from the card when state is
      `auth_required`.
- [ ] Opening Add produces a blank form with one compatible agent pre-selected by
      default (the first profile that supports the current transport); the draft
      is unchanged until the form is submitted.
- [ ] Opening Edit populates the modal with the server's current definition and
      assignments; the draft is unchanged until the form is submitted.
- [ ] The modal agent-assignment section shows all executors from the
      `supports_mcp` profile list; incompatible ones are disabled with a reason
      string derived from `codecForAgent`. For custom-JSON entries, all agents
      are selectable because transport cannot be determined.
- [ ] Attempting to submit the modal with zero agents assigned shows a validation
      error and prevents submission.
- [ ] Submitting the modal writes the complete server object (definition +
      assignments) to the draft and closes the dialog.
- [ ] Canceling or closing the modal without submitting leaves the draft in its
      pre-open state, including any assignment changes made inside the modal.
- [ ] Renaming a server through Edit replaces the entry without creating a
      duplicate.
- [ ] The JSON escape hatch continues to accept and emit `SharedMcpDraftServer[]`
      without structural change.
- [ ] Per-executor test results and auth state are preserved and accessible from
      the server card after the redesign.
- [ ] The settings save/discard bar continues to control persistence of the draft.
- [ ] The dialog is usable at a narrow viewport width (≤ 480 px) and with
      keyboard-only navigation.
- [ ] `pnpm run check` and `pnpm run lint` pass on changed files.
- [ ] Pure-function frontend tests cover transport compatibility logic
      (`isTransportCompatible`). Submit, cancel, and assignment behavior are
      verified through the T6 manual visual checklist. (No rendered-DOM test
      infrastructure exists for this surface; per Principle II, component tests
      are required only where they already exist.)
- [ ] `pnpm run format` produces no diff.

## Assumptions

- The existing `codecForAgent` helpers and `SharedMcpDraftServer` type cover
  all compatibility checks needed for the modal assignment UI without schema
  changes.
- Per-executor test results are keyed by `testKey(serverName, executor)` into
  `Record<string, SharedMcpAssignmentTestResult>`. This shape is defined entirely
  in `McpSettingsSection` state and survives the move unchanged.
- **Remote-web is not affected.** Code search confirms remote-web contains no
  MCP settings components. All affected files live in `packages/web-core/src`
  (primarily `McpSettingsSection.tsx` and `McpServerDialog.tsx`) and transitively
  `packages/local-web`. Remote-web remains out of scope.
- `McpServerDialog`'s props contract must be extended: add `profiles:
  SharedMcpProfile[]` and `initialAssignments?: BaseCodingAgent[]` inputs; change
  the result type from `{ name: string; entry: JsonValue }` to `{ name: string;
  entry: JsonValue; assignments: BaseCodingAgent[] }`. The existing `codec` prop
  is retained because it is still needed to parse and serialize the server
  definition fields (`codec.parse`, `codec.serialize`).
- Compatibility in the modal is computed dynamically from `codecForAgent(executor)`
  per profile at render time, not from the backend's `SharedMcpCompatibility`
  array (which is stale the moment the user changes transport). The
  `openDialog` callback in `McpSettingsSection` currently passes only the
  CLAUDE_CODE codec; after the change it will also pass the full `profiles` list
  so the dialog can call `codecForAgent` for each executor.
- New i18n keys will be added to all seven existing locale files (en, fr, es, ja,
  ko, zh-Hant, zh-Hans). Non-English locales may receive direct translations or
  safe neutral fallbacks; no locale file should be left with a missing key at
  runtime.
- The 10 `BaseCodingAgent` values (CLAUDE_CODE, AMP, GEMINI, CODEX, OPENCODE,
  CURSOR_AGENT, QWEN_CODE, COPILOT, DROID, GROK) represent the full set of
  possible assignments. The modal shows all profiles returned by the backend's
  `profiles.filter(p => p.supports_mcp)` list.

## Open Questions

- None at this time. All previously implied questions have been resolved against
  the codebase (see Assumptions above).
