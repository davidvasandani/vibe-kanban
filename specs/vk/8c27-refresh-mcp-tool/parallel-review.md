# Parallel Review Synthesis

## Round 1

Claude and Codex agreed that the pinned Codex protocol does not expose
per-server restart/reuse facts, cannot prove last-known-good callability for one
failed server, and needs substantially more contract coverage. Both treat the
new-process status read as only the best available next-turn proxy because
Codex exposes no inventory generation identifier.

Claude's strongest unique finding was the UI polling interval that remained
active when refresh status disappeared. Codex's strongest unique finding was
that the live control was published before `turn/start`, allowing premature
confirmation against a pre-turn inventory. Codex also identified duplicate
pending actions, stale-response risk, unknown-count coercion, and REST contract
drift.

They differed on whether current public error allow-listing is sufficient:
Claude considered public results safe, while Codex noted that the pre-existing
raw Codex log path is outside this feature's sanitizer. That broader log path
remains an open hardening item and must not be represented as fixed by this
feature.

Grok was unavailable: its installed CLI failed to construct the terminal tool
because `auto_background_on_timeout` conflicted with disabled background
execution.

### Round 1 decisions

- Publish the refresh control only after thread/turn startup.
- Report per-server restart/reuse as unknown.
- Do not synthesize `failed_retained` when Codex cannot preserve callability.
- Add bounded refresh/status RPC timeouts.
- Keep intentional removed/disabled outcomes from degrading overall status.
- Align the REST route with the specified contract.
- Fix pending polling, duplicate submission, stale response, and unknown-count
  UI behavior.

### Remaining contradiction

The pinned protocol provides neither a generation ID nor granular failed-server
retention/restart facts. VK can confirm a complete next-turn status snapshot,
but cannot prove vendor generation identity or retain a failed server's live
connection. Those fields must remain unknown/unavailable.

## Round 2

Codex confirmed that the round-one corrections removed the premature
pre-turn confirmation, invented restart facts, synthesized retention claim,
route drift, and the most serious stale UI behavior. Its strongest additional
finding was source-level: `mcpServerStatus/list` probes the effective
thread-scoped configuration with a fresh manager, so it confirms a complete
post-start capability snapshot but does not expose the live manager's inventory
generation. The implementation and documentation therefore continue to treat
restart/reuse and last-known-good callability as unknown.

Claude's second-round CLI invocation did not complete and was terminated after
it remained idle beyond the review window. Grok remained unavailable because
its CLI could not construct its terminal tool. The completed Codex response
and Claude's round-one response converged on the protocol limitation and the
remaining need for broader transport and end-to-end coverage, so the fan-out
stopped after round 2.

### Round 2 decisions

- Reject session widening from an orchestrator-scoped MCP context.
- Present `pending_next_turn` as queued work, never as success.
- Do not aggregate per-server tool counts into a misleading inventory total.
- Update the last-successful timestamp only for a fully refreshed result.
- Keep the thread-scoped status probe as the strongest available confirmation,
  while documenting that the pinned Codex protocol has no inventory generation
  identity.

### Unresolved protocol boundary

VK cannot independently prove vendor-side generation adoption through the
pinned protocol. A future Codex protocol revision should expose the adopted MCP
inventory generation and per-server replacement facts; until then VK must not
claim those facts in API or UI output.
