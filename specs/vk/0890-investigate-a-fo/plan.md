# /speckit.plan: exclusive session turns

## Context and approach
Rust coordinator, SQLite execution rows, Rust worker supervisor, Claude streaming protocol.
The direct follow-up route lacks admission serialization; comment dispatch has a private lock.
Use shared per-session admission for user follow-up and comment dispatch. Scope it
through finalization and queue handoff; nested continuation calls in the same Tokio task
reuse admission, while spawned tasks acquire their own lock. Acquire session admission
before the workspace launch gate; workspace restart bypasses it only for persistent helpers. Reserve coding
launches centrally before repository/startup awaits in services/container.rs and enforce
atomic active-row exclusion in db/models/execution_process.rs. Busy ordinary prompts enter
the existing queue; history reset remains an explicit busy error. Queue draining must use
the same ownership rules and retain claimed work on admission conflicts.

Worker execution.rs already locks the jobs map during dispatch; extend that boundary with
session identity and child/startup ownership checks, preserving execution-ID replay.
Claude's existing deny-list plus PreToolUse boundary is the supported place to deny native
peer tools in every mode. Extend those guards and context guidance; document deterministic
VK identity using workspace and session UUIDs instead of unsupported native naming knobs.

## Validation
Tests cover controlled nine-second startup, simultaneous launch attempts, busy follow-up,
worker replay/refusal and release, permission-mode deny hooks, identity stability. Run
focused Rust suites plus required format/check commands. Independent Codex review audits
cancellation, finalization and admission gaps before merge.

## Constitution check
Reuse existing queue, dispatcher, worker map and deny-hook machinery (VI); test invariants
(II); retain explicit ownership (XII) and attributed targeted delivery (LVII). Never claim
DB completion alone proves process death. No unrelated service changes.

## Risks
Queue handoff and terminal-event ordering may need lifecycle adjustments. Native Claude
harness tool enforcement is an external dependency: report the tested boundary accurately.

## Final contracts
`POST /api/sessions/:id/follow-up` preserves the started execution payload and returns
`{"queued":true}` when queued. MCP reports queued=true with no execution ID. The frontend
refreshes queue status without manufacturing a process. History-reset retries reject busy
sessions. Direct follow-ups reject an occupied queue slot rather than silently replacing accepted work. The explicit queue-edit endpoint retains replacement semantics, and failed-turn policies remain unchanged.
SQLite INSERT/UPDATE triggers reject a second running coding row without altering legacy
history. The local child store and worker spawn-pending/child-liveness checks independently
refuse a still-live predecessor even if its protocol row is terminal.
