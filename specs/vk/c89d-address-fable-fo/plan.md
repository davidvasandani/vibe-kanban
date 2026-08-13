# Implementation Plan: Close Stale Execution Follow-up Gaps

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical Context

- Backend: Rust 2024, Tokio broadcast channels/streams, SQLx/SQLite, Axum
  WebSockets, local process ownership, and cluster worker event/lease evidence.
- Frontend: React/TypeScript in `packages/web-core`; remote relay shims in
  `packages/remote-web`.
- State protocol: full JSON Patch replacement, `Ready`, then keyed patches.
- Existing status domain already includes `running`, `completed`, `failed`,
  `killed`, `interrupted`, and `indeterminate`; no generated-type expansion is
  planned.
- Constraints: Vibe Kanban only, deterministic race tests, no new dependency.

## Architecture & Approach

### 1. Restore isolated task records and guard generation

Use git history to separate the original `vk/5e1e-vk-workspace-cre` artifacts
from PR #226's `vk/3488-fix-stale-execut` artifacts currently mixed under
`specs/vk/a5f8-concat-repeating`. Preserve exact historical content first, then
repair task IDs and internal relative references only where evidence requires.

Update the SpecKit pipeline/command generation source rather than only the
checked-in generated command. The target path derives from the current task
identity and an ownership marker in the existing spec is checked before any
write. Add tests for same-owner refresh and different-owner refusal.

### 2. Make provider activity derivation authoritative

Keep one `hasRunningAttempt` helper in a neutral web-core execution module and
call it from both `useExecutionProcesses` and
`ExecutionProcessesProvider`. The provider filters dropped records before the
call. A rendered context consumer test asserts the precise value used by
`useWorkspaceExecution`, including setup scripts and the closed terminal
status table.

### 3. Generalize the lossless backend handoff

In `crates/services/src/services/events/streams.rs`, extract shared construction
for a receiver acquired before an awaited snapshot and a live
`BroadcastStream` whose lag maps to `io::Error`. Individual stream functions
retain their record filtering and snapshot mapping but cannot choose different
subscription/lag policy.

Use deterministic test gates around snapshot acquisition. The execution test
acquires the receiver, blocks the snapshot, publishes running then terminal,
releases the snapshot, and reduces snapshot/Ready/patches into client state.
The final value must be terminal. A small-capacity channel forces actual lag.

Apply this contract to execution processes, scratch, workspaces, and browser
sessions. For `MsgStore::history_plus_stream`, acquire the receiver while the
history lock/order boundary is controlled and return lag as an error; audit
normalized/stdout/stderr consumers so they terminate or resnapshot rather than
silently continue.

Axum stream routes share an error-close helper that sends code 1011 with a
resnapshot reason. A route-level socket test proves execution stream error
propagates to that close path.

### 4. Separate client readiness from local allocation

Refactor `useJsonPatchWsStream.ts` around refs for endpoint-authoritative Ready
and consecutive unhealthy attempts. Initial-data allocation remains a rendering
implementation detail. Pre-Ready failures increment the bounded error/backoff
counter, including connection-factory rejection; only `Ready` resets it.

After any Ready for the same endpoint, preserve `data` and `isInitialized`
during reconnect. A new Ready replaces/reconciles state and marks the retry
epoch healthy. Fake-WebSocket/fake-timer tests cover initial failure, retention,
open→1011 loops, increasing capped delays, and eventual recovery.

In `packages/remote-web/src/shared/lib/relay/ws.ts`, decoded close envelopes are
reported as synthetic consumer-facing close events. The raw browser socket is
closed with a legal invocation, avoiding `close(1011)` while retaining the
server code/reason exactly once for reconnect diagnostics.

### 5. Add bounded post-final-output reconciliation

Trace Codex `turn/completed`, normalized assistant patches, executor exit
signals, local child monitors, cluster ordered terminal events, and all
completion writes in `crates/local-deployment/src/container.rs` and related
services.

When normalization observes a non-empty final assistant message for a running
execution, register a 45-second reconciliation deadline tied to that execution.
Normal exit/terminal paths cancel it. The deadline task consults an
owner-specific liveness adapter:

- local: exact child/process-group and monitor/exit-signal evidence;
- cluster: exact nonterminal job, unexpired worker/job lease, and gap-free event
  state.

If positive liveness remains, reschedule within the overall policy rather than
declare success. If liveness disappears before or at the deadline, run the
existing work-preservation boundary where required and atomically/orderedly
persist `indeterminate` unless stronger completed/failed/interrupted evidence
exists. Completion writes use bounded retry and structured logs; startup
orphan reconciliation remains the durable backstop.

Tests inject paused time and fake evidence. They cover normal/delayed terminal
events, no event with dead local process, valid remote lease, expired lease,
replay gap, cancellation/interruption, and transient/exhausted DB update
failure. An integration state test feeds the resulting terminal patch through
the provider boundary and verifies Send.

## Data Model

See `./data-model.md`. Prefer in-memory reconciliation registration plus
existing durable execution/worker fields; add persistent fields only if code
inspection proves restart cannot reconstruct the required bound safely.

## Contracts

- `./contracts/snapshot-live-stream.md`
- `./contracts/execution-finalization.md`
- `./contracts/websocket-recovery.md`
- `./contracts/speckit-artifact-ownership.md`

## Research Notes

See `./research.md`. No new dependency is planned.

## Constitution Check

- II and XII: real provider, race, route, and completion boundaries are tested.
- VI: existing process ownership, status domain, worker evidence, and reconnect
  machinery are extended rather than forked.
- XV and XVIII: unknown liveness never implies success or destructive cleanup;
  preservation precedes classification.
- XXX–XXXIII: authoritative UI, lossless streams, bounded final-output
  reconciliation, and artifact ownership are the plan's primary contracts.
- No deviation is open.

## Risks & Dependencies

- Final assistant detection currently occurs during summary extraction after
  completion; an earlier normalization signal may need a narrowly scoped hook.
- Local handle disappearance and restart preservation have deliberately
  different ordering; reconciliation must not erase the startup rescue path.
- `MsgStore` history and broadcast publication must be inspected for a common
  synchronization boundary to avoid duplicates that cannot be deduplicated.
- Relay close-event synthesis must prevent the raw socket's later close event
  from emitting a second conflicting event.
- Historical artifact commits may contain unrelated files in the reused
  directory; restoration is evidence-by-file, not wholesale copying.
