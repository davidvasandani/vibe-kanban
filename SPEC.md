# Grok Executor Auto Mode

## Problem

When a Grok executor profile is configured with the `Auto` permission policy,
Vibe Kanban launches Grok with `--always-approve` and omits its interactive
approval bridge, but the ACP session itself is created in Grok's default
`Ask` mode. Grok then reports or behaves as though the session is supervised,
so the user's selected permission policy appears to revert from `Auto` to
`Ask`.

## Objective

Keep Grok ACP sessions in the permission mode selected by the Vibe Kanban
executor configuration for both initial prompts and follow-up prompts.

## Technical Requirements

1. Map Vibe Kanban's Grok `Auto` permission policy to Grok's ACP `auto` session
   mode.
2. Map Vibe Kanban's Grok `Supervised` permission policy to Grok's ACP `ask`
   session mode.
3. Apply the mapped ACP mode after every new ACP session is created, including
   sessions created for follow-up turns.
4. Preserve the existing command-line behavior:
   - `Auto` continues to add `--always-approve`.
   - `Supervised` continues to omit `--always-approve`.
5. Preserve the approval-service behavior:
   - `Auto` does not attach interactive approvals.
   - `Supervised` attaches the configured approval service.
6. Do not expose Grok ACP modes as a separate agent/persona selector in the UI;
   permission policy remains the single user-facing control.
7. Add focused regression tests that prove each permission policy selects the
   expected ACP mode and that the mode is carried by the Grok harness.

## Design

The shared ACP harness already supports an optional session mode and invokes
ACP `session/set_mode` immediately after session creation. Grok should construct
its harness with an explicit mode derived from its existing `yolo` field:

| Vibe Kanban permission | `yolo` | CLI flag | ACP session mode |
| --- | --- | --- | --- |
| Auto | `true` | `--always-approve` | `auto` |
| Supervised | `false` or unset | none | `ask` |

The mapping belongs in the Grok executor because these mode identifiers are
Grok-specific, while the ACP harness remains provider-neutral.

To make the behavior directly testable without broadening public API surface,
the Grok implementation should centralize the mode mapping in a small helper
and use it from `harness()`. Tests should validate both values and retain the
existing command construction assertions.

## Acceptance Criteria

- Starting Grok with permission policy `Auto` sends ACP
  `session/set_mode(..., "auto")` before the prompt and no approval prompt is
  shown for tool execution.
- Starting Grok with permission policy `Supervised` sends ACP
  `session/set_mode(..., "ask")` and tool approval requests remain available.
- A follow-up Grok execution reapplies the same configured ACP mode to its new
  ACP session.
- Existing Grok model selection, authentication, MCP configuration, session
  transcript replay, and command ordering remain unchanged.
- Focused Rust tests, formatting, and relevant workspace checks pass.

## Non-Goals

- Changing Grok CLI defaults outside Vibe Kanban.
- Adding new permission policies.
- Changing shared ACP mode discovery or the frontend model selector.
- Altering approval behavior for Gemini, Qwen, or other executors.

## Risks and Mitigations

- **Mode identifier drift:** Keep the Grok-specific `auto` and `ask` constants
  local and cover them with regression tests.
- **CLI/ACP disagreement:** Set both the existing CLI flag and ACP mode from the
  same `yolo` value.
- **Follow-up regression:** Reuse the same configured harness in both spawn
  paths so mode application is identical.
