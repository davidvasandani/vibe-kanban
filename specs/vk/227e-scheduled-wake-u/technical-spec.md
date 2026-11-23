# Scheduled wakeups refresh chat

Task: 227e-scheduled-wake-u

## Problem and outcome
An open session conversation must show a scheduled wakeup's new turn and streamed response without a browser reload. Investigate the wakeup dispatch → execution event → conversation subscription path and repair the missing refresh at the owning boundary.

## Scope
Vibe Kanban only. Preserve durable wakeup scheduling, queue ordering, session selection, and existing conversation history. No other services or deployment changes are assumed necessary.

## Requirements
- Automatically discover executions launched by scheduled wakeups in an already open chat.
- Stream new content and settle completed turns without duplicating history.
- Retain session isolation and reconnect behavior.
- Cover the root cause with focused regression tests and independently review the diff.

## Approach and verification
Trace existing scheduling and chat event code before choosing an implementation. Prefer the existing execution subscription mechanism over a wakeup-specific reload. Verify externally launched executions, completion, and session switching; run applicable frontend checks and repository formatting. Record prior knowledge, complete SpecKit artifacts, review, update the project wiki, and open and merge a PR.
