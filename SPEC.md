# Require bounded pollers

## Problem and scope
Vibe Kanban pollers can continue after their useful work is complete. Require an explicit automatic stopping rule for every newly created poller: a nonblank stop-condition command, a positive wall-clock time limit, or both. A stop command exits zero when polling should stop; nonzero means continue. Manual stop remains available.

## Requirements
- Enforce the rule at the HTTP boundary and expose both fields through MCP.
- Persist and return the stopping configuration alongside the command and interval.
- Evaluate the stop condition before each tick. A time limit bounds the entire poller, including a hung tick or stop condition, and terminates descendant processes.
- When both rules are supplied, either may stop the poller. Reject blank stop commands, zero limits, and missing rules with actionable errors.
- Preserve readability of historical poller records through optional fields; give legacy executions a finite fallback when recompiling them.
- Display stopping rules in the existing poller details and update agent guidance.
- Add meaningful validation and process-lifecycle tests, regenerate shared types, format, and review independently.

## Acceptance
A request without either rule fails before spawning. Stop-command and deadline pollers terminate automatically; both rules work together. Restart recovery preserves deadlines rather than extending lifetimes. Existing records remain deserializable. No other service is changed.
