# Technical specification: reliable Claude background-Bash denial

**Task:** `VAS-540` / `vk/5cd1-debug-this-vk-ba`

## Problem

Vibe Kanban registers the SDK callback
`DENY_BACKGROUND_BASH_CALLBACK_ID` as a Claude Code `PreToolUse` hook. When
Claude requests `Bash` with `run_in_background: true`, some executions print a
large minified CLI excerpt followed by:

```text
Error in hook callback DENY_BACKGROUND_BASH_CALLBACK_ID: ...
error: Stream closed
```

The denial itself is intentional: processes backgrounded inside an agent turn
are reaped with that turn, while a Vibe Kanban poller is the supported durable
replacement. The defect is the failed control-protocol round trip. A callback
request accepted from Claude must receive its response before Vibe Kanban can
consider the protocol input stream disposable.

## Objective

Make the background-Bash denial reliable at execution boundaries so Claude
receives the actionable `spawn_poller` guidance without emitting a control
stream error, losing the current request, or weakening the prohibition on
in-turn background work.

## Scope

- Claude executor structured-input/control-protocol lifecycle.
- The `DENY_BACKGROUND_BASH_CALLBACK_ID` hook and closely related trailing
  hook behavior needed to correct the shared lifecycle bug.
- Focused executor tests and durable Vibe Kanban documentation.
- Vibe Kanban source only. No other service or deployment changes are needed
  unless investigation proves that `modules/vibe-kanban-rebuild.nix` owns the
  faulty lifecycle; such a finding must be documented before editing it.

## Required behavior

1. Every parsed Claude control request is either answered successfully while
   the protocol input stream is open or fails through an explicit executor
   error path; it is never abandoned merely because a result, cancellation,
   timeout, or stdout EOF races with it.
2. A `DENY_BACKGROUND_BASH_CALLBACK_ID` request whose input explicitly has
   `tool_input.run_in_background: true` receives a `PreToolUse` deny response
   containing the existing actionable `spawn_poller` guidance.
3. Foreground Bash remains permitted or approval-routed according to the
   selected permission mode.
4. Terminal-result and cancellation handling still let Claude exit promptly;
   the fix must not keep executions alive indefinitely.
5. The existing protection against late Stop-hook `Stream closed` failures and
   spurious zero-turn resume results remains intact.
6. Protocol write failures are observable in Vibe Kanban logs and propagate
   where ownership permits; successful turn completion must not mask a failed
   required hook response.

## Verification requirements

- Add a deterministic protocol-level regression test that reproduces the
  relevant ordering using mocked child stdin/stdout or an equivalent duplex
  transport; a value-only unit test of the denial JSON is insufficient.
- Cover at least the background denial response and the terminal-boundary
  ordering that previously closed the stream.
- Retain the current unit coverage for callback routing in auto, supervised,
  and plan modes.
- Run focused executor tests, formatting, and repository checks proportionate
  to the files touched.
- Verify the adopted lifecycle against the pinned Claude Code artifact and
  relevant upstream SDK/CLI primary-source behavior.

## Success criteria

- The reported VAS-540 sequence no longer emits `Error in hook callback
  DENY_BACKGROUND_BASH_CALLBACK_ID` or `Stream closed`.
- Claude sees a normal tool denial explaining that durable work belongs in
  `spawn_poller`.
- No background Bash process is admitted, and foreground Bash behavior does
  not regress.
- New regression coverage fails against the faulty lifecycle and passes with
  the correction.

## Non-goals

- Allowing Claude's native background Bash or polling tools.
- Changing VK poller scheduling, persistence, or UI behavior.
- Updating Claude Code merely to avoid fixing an application-owned protocol
  race.
- Modifying any hosted service other than Vibe Kanban.

## Initial investigation hypotheses

The error string originates in Claude Code's structured-input request broker
when its inbound permission/control stream closes before a pending callback is
resolved. Existing Vibe Kanban code already has a fixed 500 ms post-result
grace for late Stop hooks, but the new background-Bash hook exercises the same
contract at a different ordering boundary. Planning must determine whether the
root cause is a fixed-duration grace window, detached reader-task ownership,
stdout EOF handling, or a pinned-CLI regression before selecting the fix.
