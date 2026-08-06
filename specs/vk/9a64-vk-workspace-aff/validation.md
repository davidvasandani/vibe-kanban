# Validation: Workspace Server Affinity and Migration

## Automated

- `cargo test -p db placement_is_sticky_and_transitions_forward` — passed; covers reserve, forward placement transitions, compare-and-set reassignment, requested affinity, and stale-update refusal.
- `cargo check -p server` — passed after adding the affinity route, durable operation migration, deterministic continuation execution identity, summary types, and generated contract registrations.
- `pnpm run generate-types` — passed; generated affinity request/outcome/response and summary types.
- `pnpm --filter @vibe/web-core run check` — passed.
- `pnpm --filter @vibe/ui run check` — passed.
- `vitest run src/shared/lib/workerPlacement.test.ts` — 3 tests passed; covers eligible, offline, unhealthy-mount, expired-lease, and executor-capability cases.
- Independent `codex review --base origin/main` — final pass found no discrete actionable correctness issues and independently passed `cargo check -p server`.
- Locale JSON parse check across all seven `common.json` files — passed.
- `pnpm run format` — passed.

## Scenario inspection

- Local placement: bulk summary classifies `local`; accordion header/body render the local server informationally and omit the selector.
- Automatic placement: selector sends null requested worker; backend runs the existing scheduler immediately and returns a concrete placement.
- Explicit placement: unavailable workers remain identifiable; only a current unavailable worker remains selectable, and selecting the effective current value is a no-op.
- Running migration: provisional selection opens confirmation; cancel resets the selector; confirm carries a stable operation ID through retries.
- Stale running state: a backend confirmation-required conflict triggers the same confirmation flow instead of mutating immediately.
- Partial restart failure: backend returns `restart_failed` with the durable new placement and the UI reports that precise state.
- Concurrent/retried migration: one active DB claim per workspace plus operation-ID/execution-ID identity prevents a second continuation.

## Environment limitation

This isolated worktree has no live multi-worker cluster attached, so a real two-node dispatch/migration exercise cannot be performed here. The coordinator/worker integration remains covered by existing cluster dispatch and cancellation tests; deployment validation should exercise one stopped and one running workspace after rollout.
