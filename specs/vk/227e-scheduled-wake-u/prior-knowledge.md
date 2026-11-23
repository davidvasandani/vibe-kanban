# Prior knowledge
Searched wiki/ and docs/knowledge-base/ for wakeups, conversation loading and snapshot streams.
- wiki/durable-session-wakeups.md: timers reuse the durable delivery dispatcher and ordinary follow-up execution; do not add a second launch path. Due sweep is five seconds; active sessions defer admission.
- wiki/awaited-stream-settlement.md: useExecutionProcesses shares a host-pinned discovery socket; completed logs load via HTTP and running logs via WebSocket. Preserve bounded settlement and session/host identity.
- wiki/conversation-entry-aggregation.md: turn boundaries come from real user_message entries; do not invent synthetic messages to force a refresh.
- docs/knowledge-base/authoritative-snapshot-stream-handoffs.md: reconcile authoritative process snapshots and streams without a gap or premature settled assumption.
