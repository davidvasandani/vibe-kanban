# Clarifications: Close Stale Execution Follow-up Gaps

## 1. What is positive execution liveness?

**Decision:** Use the evidence channel belonging to the execution owner.

- Local executions: a registered child/process-group that an OS liveness probe
  confirms, or a still-live executor exit-signal/monitor task tied to that exact
  execution. A database `running` row, missing handle, or final assistant message
  is not positive liveness.
- Cluster executions: the exact execution worker job remains nonterminal, its
  assigned worker/job lease is unexpired, and ordered worker polling has not
  reported a replay gap or terminal event. General worker metrics are not
  liveness evidence.

One reconciliation state machine may express the shared decisions, but local
and cluster adapters supply their owner-specific evidence.

## 2. What is the recovery interval?

**Decision:** Use a configurable/testable 45-second no-terminal-evidence bound
after final assistant output, with immediate normal completion whenever the
ordinary terminal event arrives.

The cluster's normal lease is 30 seconds. Forty-five seconds permits one full
lease plus scheduling/polling margin without allowing a human-scale indefinite
spinner. Positive liveness observed within reconciliation prevents premature
classification; loss of positive liveness permits immediate classification
rather than requiring the whole interval. Tests use paused time or an injected
clock, never wall-clock sleeps.

## 3. Which fallback terminal status is truthful?

**Decision:** Use `indeterminate` when final assistant output exists but neither
successful/failed exit nor a user/system interruption can be proven. Use
`interrupted` only when interruption evidence exists, `failed` only with failure
evidence, and `completed` only with successful terminal evidence.

Final text arms reconciliation and improves diagnostics; it does not convert an
unknown exit into success.

## 4. How is relay close metadata preserved?

**Decision:** Treat the decoded relay close envelope as the authoritative close
metadata for the shim's consumer-facing `CloseEvent`. Do not attempt to originate
reserved code `1011` via browser `WebSocket.close(code)`. Close the underlying
relay transport using a browser-legal call, then emit/retain the decoded server
code, reason, and cleanliness on the shim-facing event exactly once. The
consumer therefore receives `1011` plus the resnapshot reason and still follows
its unexpected-close reconnect path.

## Remaining questions

None.
