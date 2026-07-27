# Clarifications: Active MCP Refresh

`/speckit.clarify` resolved the specification's open questions from the pinned
executor protocols and existing VK lifecycle.

## C1. Supported executor

The first implementation targets Codex app-server sessions. The pinned
`codex-app-server-protocol` includes `config/mcpServer/reload`, documented to
reload MCP config from disk and queue refresh for loaded threads on their next
active turn. It also exposes `mcpServerStatus/list` with tools and authentication
state.

No equivalent live, confirmable reload contract is established in this
repository for Claude, OpenCode, ACP/Grok, Gemini, Cursor, Amp, Copilot, Droid,
or Qwen. They return `unsupported`; an independent `mcp_test.rs` probe is not
substituted for live adoption.

## C2. Rollout scope

Capability-gated first release is acceptable. The UI remains visible for active
sessions and states when the selected executor cannot refresh in place. This is
more truthful than silently restarting or probing.

## C3. Metadata durability

Last-successful time and per-server status are retained with the live session
coordinator. They do not survive a VK backend restart in the first release
because the live executor control channel itself is process-local.

## C4. Active calls

Codex's contract queues reload for each loaded thread and applies it at the next
active turn. VK uses that serialization point instead of disrupting the current
turn. Concurrent refresh submissions are rejected as retryable busy while the
queued generation awaits confirmation.

The initial action reports `pending_next_turn`, not success. After the next turn
starts and status enumeration confirms the adopted server/tool set, the
generation becomes successful and UI metadata updates.

## Remaining non-blocking limitation

The Codex reload response is empty and acknowledges queuing rather than adoption.
VK must correlate the next-turn live status before declaring success. Capability
counts beyond tools remain optional when the pinned status detail does not expose
them.
