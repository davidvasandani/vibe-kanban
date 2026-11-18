# Implementation Plan: Automatic exponential backoff for model-capacity errors

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context
Rust workspace (axum, sqlx on SQLite, tokio), React + TypeScript frontend,
ts-rs generated types. The deployment is a coordinator with cluster workers;
both local exit monitoring and worker reconciliation end a turn through
`ContainerService::finalize_task` (`crates/services/src/services/container.rs`).
No new dependency: jitter uses `rand`, already a workspace dependency of
`services` (verified in step T001; otherwise derive it from a UUID's bytes).

## Architecture & Approach
The retry is a durable wake-up with two extra columns, scheduled by a new
event consumer and delivered by the existing dispatcher.

1. **Policy (FR-1..4)** — new `crates/services/src/services/capacity_backoff.rs`,
   pure functions: `is_capacity_error`, `turn_ended_with_capacity_error`,
   `backoff_delay`, `with_jitter`, `next_attempt`, `retry_message`.
2. **Config (FR-12)** — `CapacityBackoffConfig` in
   `crates/services/src/services/config/versions/v8.rs`, a defaulted field on
   `Config`, re-exported in `config/mod.rs`, declared in
   `crates/server/src/bin/generate_types.rs`.
3. **Storage (FR-7..10)** — migration
   `crates/db/migrations/20261118000000_capacity_retry_wakeups.sql` and
   `crates/db/src/models/wakeup.rs`: new fields, `insert` with named columns
   and an agent-only budget count, `insert_capacity_retry`,
   `capacity_attempt_for_execution`, and a capacity branch in `message`.
4. **Scheduling (FR-1, 2, 4, 7, 12)** — new
   `crates/server/src/capacity_backoff.rs` subscribes to
   `turn_finalized_sender`, wired in `crates/server/src/startup.rs` beside
   `error_remediation::spawn` and pinned in
   `crates/server/tests/startup_wiring.rs`. The handler identifies the turn as
   the session's latest coding-agent turn and requires
   `SessionTurnBoundary::finalized_turn` to name it, so a delayed event cannot
   act on a newer turn.
5. **Delivery and yielding (FR-5, 6)** —
   `crates/server/src/issue_comment_delivery.rs::dispatch_session_admitted`
   cancels superseded capacity rows and lets valid ones resume a stopped
   turn. `crates/server/src/wakeups.rs::reconcile` applies the same
   cancellation on every sweep. Both call one helper,
   `wakeups::capacity_retry_superseded`.
6. **Remediation exclusion (FR-13)** — `crates/server/src/error_remediation.rs`
   skips a failed turn that ended with a capacity error, before reserving.
7. **UI (FR-11, 12)** — `packages/web-core/src/pages/workspaces/WakeupRow.tsx`
   and its test; a checkbox in
   `packages/web-core/src/shared/dialogs/settings/settings/GeneralSettingsSection.tsx`;
   strings in `packages/web-core/src/i18n/locales/*/settings.json`.
8. **Docs** — a section in `docs/` describing the behavior and the setting.

## Data Model
See `./data-model.md`.

## Contracts
See `./contracts/capacity-retry.md`.

## Research Notes
See `./research.md`.

## Constitution Check
- **LX**: evidence-driven trigger, bounded exponential chain, idempotent by a
  unique index, supersession checked under session admission, no unarchive,
  separate budget, visible and cancellable, config read when acting,
  excluded from remediation. Honored by steps 1–6.
- **LVII**: delivered through the shared dispatcher at a recorded boundary;
  launch evidence and the uncertain-launch rule are unchanged.
- **XLI**: not self-spawned work; nothing is filed or started.
- **XXVIII**: the schedule is a database row written in one transaction.
- **XII**: the consumer holds no lock across I/O; the dispatcher's existing
  admission lock covers the claim.
- **II**: every step has tests. **III/VI**: no new scheduler or table.
- Generated types are regenerated, never edited.
No deviation.

## Risks & Dependencies
- **Log lag.** On a worker the error entry could reach the coordinator after
  finalization. A one-second settle delay precedes the read; a miss means no
  retry, which is the pre-feature behavior.
- **False positive.** A non-capacity error entry containing the phrase would
  cause a bounded retry chain. Only error entries at the end of the turn count.
- **Other executors.** Only the Codex phrasing is matched; others are
  unaffected until a pattern is added.
- **Existing `wakeups` rows.** New columns are nullable, so old rows are agent
  wake-ups by definition.
- Live verification against a real outage is not possible on demand.
