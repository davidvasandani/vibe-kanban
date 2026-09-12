# Prior knowledge: Claude background-hook stream lifecycle

**Task:** `VAS-540` / `vk/5cd1-debug-this-vk-ba`

The project knowledge base was searched for Claude hooks, background Bash,
pollers, control requests, stream closure, and executor process lifecycle.

## Directly relevant knowledge

### `wiki/vk-pollers.md`

- The `Bash(run_in_background: true)` PreToolUse hook is the load-bearing
  Claude guard. Denying background-related tool names alone is insufficient
  because Claude can read a background task's output through ordinary `Read`.
- The predicate must remain conservative: only explicit boolean `true` is
  denied. Missing, malformed, and false values fall through so foreground Bash
  cannot be broken.
- The denial must run before auto-approval and must name `spawn_poller`, giving
  the agent a usable durable replacement.
- Claude wire identifiers were verified against the pinned native artifact,
  not the npm stub's schema-title declarations.

### `wiki/agent-process-lifecycle.md`

- Claude is a natural-exit executor: one turn maps to one child-process
  lifetime, and VK expects it to exit rather than receiving an independent
  protocol turn-completion signal.
- The same page records the two process-group kill points at execution
  finalization. This confirms why native in-turn background work remains
  unsupported and why weakening the denial is not an acceptable workaround.
- Lifecycle shortcuts must preserve all handoffs owned by the normal path. For
  this bug, a parsed control request and its response are such a handoff: stream
  teardown must not bypass it.
- Output draining needs a bound because inherited descriptors can prevent EOF,
  but a liveness bound must be attached to terminal evidence rather than
  silently discarding protocol obligations.

### `docs/knowledge-base/authoritative-snapshot-stream-handoffs.md`

- A stream consumer must establish authoritative state before relying on the
  incremental tail, and ownership transfer must avoid gaps. Applied here, the
  protocol reader must retain ownership until requests accepted from the stream
  have been answered; a timer is not proof that the handoff completed.
- Restart/recovery behavior should be level-triggered from current authority,
  not inferred from a transient event. This supports testing explicit protocol
  states rather than adding another timing-only sleep.

### `docs/knowledge-base/claude-log-normalization.md`

- Claude emits several structured message shapes on stdout, and parser changes
  must avoid turning transport/control messages into user-visible conversation
  content.
- Tool/progress metadata is not terminal evidence by itself. Regression tests
  should exercise the structured protocol boundary without depending on the
  chat log renderer.

## Existing source history

- PR #56 (`e072e906`) previously fixed a related Stop-hook failure by keeping
  stdin open for a fixed 500 ms after a terminal result and by ignoring a known
  spurious zero-turn resume result. VAS-540 shows that the general control
  stream invariant is not fully captured by that Stop-specific grace period.
- PR #252 (`79941275`) introduced
  `DENY_BACKGROUND_BASH_CALLBACK_ID`. Its current unit tests validate callback
  routing and JSON values, but they do not test the bidirectional transport
  lifetime.

## Upstream primary-source evidence

Firecrawl Developer search found Anthropic SDK reports with the same invariant:

- `anthropics/claude-agent-sdk-python#730` attributes
  `Tool permission stream closed before response received` to the input side
  of the bidirectional protocol closing while hooks are still active.
- `anthropics/claude-agent-sdk-typescript#369` reports that closing shared
  stdin on an early result breaks later `canUseTool` and hook round trips.
- `anthropics/anthropic-sdk-typescript#840` records SessionEnd hooks firing
  after the SDK has already closed streams.

These reports corroborate the error classification, but the implementation
must still reproduce the ordering in VK's own Rust protocol adapter and verify
the pinned CLI artifact before adopting a specific workaround.

## Consequences for specification and planning

1. Preserve the background denial; fix transport ownership instead.
2. Prefer an explicit “terminal result plus no outstanding protocol work”
   condition over increasing the 500 ms constant.
3. Treat EOF, cancellation, terminal results, and in-flight callback responses
   as distinct lifecycle signals in tests.
4. Keep changes inside the Vibe Kanban repository unless direct evidence points
   to its governing Nix module.
5. Add protocol-level regression coverage; value-only hook tests cannot prove
   that Claude received the response.
