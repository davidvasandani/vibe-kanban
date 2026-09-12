# Implementation Plan: Vibe Kanban Soft Restarts

**Spec**: `./spec.md`
**Status**: Implemented

## Technical Context

The Rust coordinator owns SQLite/product state, while cluster workers already own agent process groups, bounded ordered journals, approvals, cancellation, preview proxying, and PTYs. `LocalContainerService` reconciles worker evidence and excludes worker-owned rows from coordinator orphan cleanup. The Nix deployer currently undermines this boundary by unconditionally restarting workers after release distribution. The React workspace streams already retry, but their retry effect cleanup discards initialized data.

## Architecture & Approach

### Stable-owner release drain

Extend `ExecutionSupervisor` with an authoritative active count. Add a shared admission-drain atomic to worker runtime. SIGUSR1 persists a marker then closes admission; SIGUSR2 removes it then reopens admission. Dispatch checks existing execution identity before the gate so uncertain same-digest retries remain idempotent, while new execution return retryable 503 responses.

Worker `/health` publishes the gate and counts plus a conservative derived `drain_safe`. The deployer signals drain, waits for acknowledgement, and only activates when safe. The marker carries drain state through restart/health gate, preventing new work before success/rollback is known.

### Coordinator restart behavior

No new protocol is needed: worker ownership, event replay, registration retry, reconciliation-before-cleanup, and worker-row orphan exclusion already provide the soft coordinator boundary. Add a regression test showing execution/output continues while coordinator polling is absent.

### Browser behavior

Separate endpoint-change reset from same-endpoint retry cleanup in `useJsonPatchWsStream`. Preserve initialized data and readiness during retry, add bounded jitter to backoff, surface workspace connection state through the existing provider, and render an additive accessible status in `SharedAppLayout`.

## Data Model

See `./data-model.md`. No SQLite migration.

## Contracts

- `./contracts/supervisor-control.md`
- `./contracts/frontend-reconnect.md`

## Research Notes

See `./research.md`. The core conclusion is that the existing worker is already the requested bootstrap; the correct implementation protects its lifetime rather than building another process supervisor.

## Constitution Check

- Reuses shipped cluster ownership/replay/reconciliation machinery (VI, XVIII, XXI).
- Makes drain a single evidence-backed admission boundary and preserves idempotent retries (XII, XXII).
- Derives lifecycle safety from owner registries, not metrics/process guesses (XIX).
- Fails safe for old, busy, or unreadable workers (XV).
- Keeps rendered state and explicitly communicates recovery (II, IV).

No deviation remains. Standalone local supervision is explicitly out of scope rather than falsely presented as preserved.

## Risks & Mitigations

- **Dispatch during drain**: rejected with retryable 503; accepted identity retries still succeed.
- **Restart opens admission early**: persisted marker initializes candidate drained.
- **Old worker interprets SIGUSR1 fatally**: distributor checks the new health field before signalling and defers old binaries.
- **Health failure/rollback**: marker remains across restart; rollback resumes only after restoring prior release.
- **Multiple stream retries synchronize**: ±20% jitter with an 8-second cap.

## Verification

Worker unit tests cover active counts, drain derivation, dispatch idempotency/refusal, and coordinator polling gaps. Web-core tests cover retry bounds, retained snapshots, and banner visibility. Type checking, worker binary check, Nix parse/evaluation, formatting, lint, diff checks, independent review, and knowledge-base update complete the task.
