# Technical Specification: Grok Executor Support

## Summary

Add xAI's official Grok Build coding agent as a first-class Vibe Kanban executor. Users must be able to configure, discover, select, launch, and continue Grok sessions through the same executor profile and task interfaces used by the existing coding agents.

## Goals

- Introduce `GROK` as a stable serialized `BaseCodingAgent`/`CodingAgent` variant.
- Provide a Grok executor configuration with prompt suffixing, optional model selection, command overrides, additional arguments, and environment variables.
- Launch the official `grok` CLI headlessly and consume its machine-readable streaming output.
- Preserve Grok session identifiers so follow-up turns resume the prior conversation.
- Normalize Grok output into Vibe Kanban's standard log entries, including assistant text, reasoning when available, tool activity, errors, completion, and token/context usage when reported.
- Detect whether Grok is installed/authenticated and expose useful setup/authentication guidance.
- Integrate Grok with generated TypeScript types, executor settings, selectors, icons/labels, profile persistence, MCP configuration, and any exhaustive backend/frontend mappings.
- Cover command construction, event parsing/normalization, availability, session continuation, and serialization with automated tests.

## Non-goals

- Implement or proxy the xAI model API directly.
- Store or display plaintext xAI credentials.
- Add Grok-specific billing, account, or model-management UI beyond the standard executor controls.
- Reimplement the Grok terminal UI.
- Support the unrelated community `grok-cli` package; this integration targets xAI's official Grok Build CLI.

## Technical Approach

### Executor model

Add a `Grok` configuration type and a `CodingAgent::Grok` variant. The configuration should follow existing executor conventions and support:

- `append_prompt`
- optional `model`
- `base_command_override`
- `additional_params`
- `env`

The default command must use the installed official `grok` executable. Overrides remain available for development and nonstandard installations. No API key value is persisted by Vibe Kanban; authentication is inherited from the user's Grok login or execution environment (for example `XAI_API_KEY`).

### Process protocol

Use Grok's documented headless interface (`-p` with streaming JSON output) for initial turns. Select documented noninteractive/permission flags needed for unattended coding work. Follow-up turns must use the CLI's supported session-resume mechanism, with the Vibe Kanban session ID sourced from Grok's structured output rather than inferred from terminal text.

The implementation must tolerate unknown event fields/types for forward compatibility, surface malformed or fatal output as useful executor errors, and avoid leaking credentials in logs.

### Log normalization

Map Grok's structured stream to the existing normalized log model:

- assistant messages -> assistant/message entries
- reasoning/thinking -> reasoning entries when supplied
- tool starts/results -> normalized tool/action entries where inputs are safe and understood; otherwise generic tool entries
- session metadata -> stored agent session ID
- usage -> context/token usage when supplied
- fatal/error/result events -> error/completion state

Parser fixtures should be derived from documented or locally captured, sanitized output.

### Discovery and setup

Availability checks should distinguish an executable that is missing from an executable that exists but requires login. Setup messaging should direct users to the official Grok Build installation and `grok login`/device authentication flow. Dynamic model discovery should be implemented only if the CLI provides a stable machine-readable command; otherwise the standard free-form model setting is sufficient.

### MCP compatibility

Grok must participate in shared MCP assignment/configuration using the official CLI's supported MCP configuration format and path. If the CLI owns configuration mutation, Vibe Kanban must preserve unrelated user configuration and use the project's existing MCP adapter/merge abstractions.

### Frontend and generated contracts

Regenerate shared TypeScript bindings from Rust after adding the variant. Update exhaustive UI maps and settings schemas so Grok has a human-readable label and icon, appears in executor selectors/configuration screens, and round-trips profiles without hand-editing generated files.

## Acceptance Criteria

1. A user can create and persist a Grok executor profile in Vibe Kanban.
2. Grok appears consistently in executor selectors and settings with an appropriate label/icon.
3. Starting a task with Grok invokes the official CLI headlessly in the task worktree with the configured model, arguments, environment, and appended prompt.
4. Structured Grok output renders incrementally in the task log and terminal failures become visible failures rather than silent hangs.
5. A successful initial turn records a Grok session ID; a follow-up resumes that same session.
6. Missing installation and missing authentication produce actionable availability/setup states without exposing secrets.
7. Grok can receive compatible shared MCP server configuration without overwriting unrelated configuration.
8. Generated Rust/TypeScript contracts include `GROK`, and all exhaustive matches compile.
9. Focused executor/parser/configuration tests pass, followed by repository formatting, type checks, and relevant Rust/frontend tests.
10. Existing executor behavior and stored profiles remain backward compatible.

## Risks and Open Research Items

- Confirm the exact official CLI resume, approval, streaming-event, model-list, and MCP configuration contracts against the installed/current CLI documentation during SpecKit research.
- The official CLI is newly released and its event schema may evolve; parsing must be defensive.
- Authentication detection may not have a dedicated stable status command; avoid network-dependent probing where possible.
- Adding an enum variant creates broad compile-time integration work across backend and frontend exhaustive mappings.

## Verification

- Unit tests for command arguments and override precedence.
- Fixture-based tests for streaming events, unknown events, session extraction, tool mapping, errors, and usage.
- Serialization/profile round-trip tests for `GROK`.
- MCP adapter/config-path tests.
- Regenerate and verify shared TypeScript types.
- Run `pnpm run format`, targeted Rust tests, frontend checks, and the broad repository checks appropriate to the final diff.
